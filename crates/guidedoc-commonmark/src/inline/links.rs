//! Links and images: the bracket stack, inline destinations, and reference links.

use guidedoc::{Fail, NodeKind};

use super::{Inline, Txt};
use crate::inline::entities::push_unescaped;
use crate::refs::{lookup_ref, scan_link_destination, scan_link_label, scan_link_title, spnl};

/// An opening `[` or `![` waiting for its `]`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bracket {
    item: usize,
    prev_delim: Option<usize>,
    /// Subject offset of '['.
    index: usize,
    prev: Option<usize>,
    image: bool,
    active: bool,
    /// Another bracket opened after this one.
    after: bool,
}

/// A matched link destination and title, in the parser's buffer.
#[derive(Debug, Clone, Copy, Default)]
struct Target {
    dest: Txt,
    title: Option<Txt>,
}

impl Inline<'_, '_, '_, '_> {
    fn add_bracket(&mut self, item: usize, index: usize, image: bool) {
        if let Some(top) = self.top_bracket {
            self.brackets[top].after = true;
        }
        self.brackets.push(Bracket {
            item,
            prev_delim: self.top_delim,
            index,
            prev: self.top_bracket,
            image,
            active: true,
            after: false,
        });
        self.top_bracket = Some(self.brackets.len() - 1);
    }

    pub(super) fn parse_open_bracket(&mut self) -> bool {
        let start = self.pos;
        self.pos += 1;
        let item = self.add_text_item(start, self.pos);
        self.add_bracket(item, start, false);
        true
    }

    pub(super) fn parse_bang(&mut self) -> bool {
        let start = self.pos;
        self.pos += 1;
        if self.subject.as_bytes().get(self.pos) == Some(&b'[') {
            self.pos += 1;
            let item = self.add_text_item(start, self.pos);
            self.add_bracket(item, start + 1, true);
        } else {
            self.add_text_item(start, self.pos);
        }
        true
    }

    /// Parses "(destination title)" after a closing bracket.
    fn inline_target(&mut self) -> Result<Option<Target>, Fail> {
        let s = self.subject.as_bytes();
        if s.get(self.pos) != Some(&b'(') {
            return Ok(None);
        }
        let dest = scan_link_destination(s, spnl(s, self.pos + 1));
        let reached = match dest {
            Ok(r) => r.next,
            Err(next) => next,
        };
        self.h.spend(reached.saturating_sub(self.pos).max(1))?;
        let Ok(dest) = dest else { return Ok(None) };
        let mut pos = spnl(s, dest.next);
        let mut title = None;
        if pos > dest.next {
            let r = scan_link_title(s, pos);
            let reached = match r {
                Ok(r) => r.next,
                Err(next) => next,
            };
            self.h.spend(reached.saturating_sub(pos).max(1))?;
            if let Ok(r) = r {
                pos = r.next;
                title = Some(r);
            }
        }
        let pos = spnl(s, pos);
        if s.get(pos) != Some(&b')') {
            return Ok(None);
        }
        self.pos = pos + 1;
        let subject = self.subject;
        let mut t = Target { dest: self.unescaped(&subject[dest.start..dest.end]), title: None };
        if let Some(r) = title {
            t.title = Some(self.unescaped(&subject[r.start..r.end]));
        }
        Ok(Some(t))
    }

    /// Appends `s` with escapes and entities decoded to the buffer.
    fn unescaped(&mut self, s: &str) -> Txt {
        let start = self.text.len();
        push_unescaped(&mut self.text, s);
        Txt { start, len: self.text.len() - start }
    }

    pub(super) fn parse_close_bracket(&mut self) -> Result<bool, Fail> {
        self.pos += 1;
        let start = self.pos;
        let Some(opener) = self.top_bracket else {
            self.add_text_item(start - 1, start);
            return Ok(true);
        };
        let br = self.brackets[opener];
        if !br.active {
            self.add_text_item(start - 1, start);
            self.top_bracket = br.prev;
            return Ok(true);
        }
        let mut target = self.inline_target()?;
        if target.is_none() {
            target = self.reference_target(br, start)?;
        }
        let Some(target) = target else {
            self.top_bracket = br.prev;
            self.pos = start;
            self.add_text_item(start - 1, start);
            return Ok(true);
        };
        self.make_link(opener, target)?;
        Ok(true)
    }

    /// Looks up a full, collapsed, or shortcut reference after `]` at `start - 1`.
    fn reference_target(&mut self, br: Bracket, start: usize) -> Result<Option<Target>, Fail> {
        let s = self.subject;
        let n = scan_link_label(s.as_bytes(), self.pos);
        self.h.spend(n.max(1))?;
        let label = if n > 2 {
            Some(&s[self.pos + 1..self.pos + n - 1])
        } else if !br.after {
            Some(&s[br.index + 1..start - 1])
        } else {
            None
        };
        if n > 0 {
            self.pos += n;
        }
        let Some(label) = label else { return Ok(None) };
        let Some(r) = lookup_ref(self.h, label) else { return Ok(None) };
        let (dest, title) = (r.dest.clone(), r.title.clone());
        let dest = self.push_text(&dest);
        let title = title.map(|t| self.push_text(&t));
        Ok(Some(Target { dest, title }))
    }

    fn make_link(&mut self, opener: usize, target: Target) -> Result<(), Fail> {
        let br = self.brackets[opener];
        let first = br.index - usize::from(br.image);
        let kind = if br.image { NodeKind::Image } else { NodeKind::Link };
        let link = self.new_item(kind, first, self.pos);
        self.items[link].dest = target.dest;
        self.items[link].title = target.title;
        let mut tmp = self.items[br.item].next;
        while let Some(t) = tmp {
            self.h.spend(1)?;
            let next = self.items[t].next;
            self.unlink_item(t);
            self.append_child(link, t);
            tmp = next;
        }
        self.append_child(0, link);
        self.process_emphasis(br.prev_delim)?;
        self.top_bracket = br.prev;
        self.unlink_item(br.item);
        if br.image {
            return Ok(());
        }
        // Links may not contain links: deactivate earlier link openers. Openers below an
        // inactive one were deactivated together with it.
        let mut o = self.top_bracket;
        while let Some(i) = o {
            self.h.spend(1)?;
            o = self.brackets[i].prev;
            if self.brackets[i].image {
                continue;
            }
            if !self.brackets[i].active {
                break;
            }
            self.brackets[i].active = false;
        }
        Ok(())
    }
}
