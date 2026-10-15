//! Section titles: one adornment per level, so a reader nests them as the tree does.

use guidedoc::{NodeId, NodeKind, ROOT};

use crate::escape::{starts_construct, width};
use crate::state::Writer;

/// Underline characters by level, in the order the Python documentation conventions
/// and this project use: `=` for the document title, then `-`, `~`, `^`, ...
const ADORNMENTS: &[char] = &['=', '-', '~', '^', '"', '\'', '+', '`', '#', '*', ':', '.', '_'];

impl Writer<'_, '_> {
    /// Decides where section adornments start: after the document title's and
    /// subtitle's, when the reader promoted them.
    pub fn scan_titles(&mut self) {
        let mut children = self.doc.children(ROOT);
        let first = children.next().map(|c| self.kind(c));
        let second = children.next().map(|c| self.kind(c));
        self.adornment_base = match (first, second) {
            (Some(NodeKind::Title), Some(NodeKind::Subtitle)) => 2,
            (Some(NodeKind::Title), _) => 1,
            _ => 0,
        };
        if self.adornment_base > 0 {
            self.moved = self.moved_run();
        }
    }

    /// The comments and targets the reader moved from before the title to after the
    /// docinfo (Docutils' DocTitle transform): the run after the title, subtitle,
    /// docinfo, dedication, and abstract that holds a target naming the title.
    fn moved_run(&self) -> Vec<NodeId> {
        let doc = self.doc;
        let front = |c: NodeId| match self.kind(c) {
            NodeKind::Title | NodeKind::Subtitle => true,
            NodeKind::FieldList => doc.children(c).any(|f| self.custom(f, "docinfo").is_some()),
            NodeKind::Topic => doc.has_class(c, "dedication") || doc.has_class(c, "abstract"),
            _ => false,
        };
        let run: Vec<NodeId> = doc
            .children(ROOT)
            .skip_while(|&c| front(c))
            .take_while(|&c| matches!(self.kind(c), NodeKind::Comment | NodeKind::Target))
            .collect();
        let names_title =
            run.iter().any(|&t| self.kind(t) == NodeKind::Target && self.names_earlier(t));
        if names_title { run } else { Vec::new() }
    }

    /// Writes the moved nodes up to and including the target naming `title` (all of
    /// them for `None`), in order.
    fn write_moved(&mut self, title: Option<NodeId>) {
        let end = match title {
            Some(t) => self.moved.iter().position(|&m| {
                self.kind(m) == NodeKind::Target
                    && self.doc.link_info(m).is_some_and(|l| l.resolved == t)
            }),
            None => Some(self.moved.len().saturating_sub(1)),
        };
        let Some(end) = end.filter(|_| !self.moved.is_empty()) else { return };
        let written: Vec<NodeId> = self.moved.drain(..=end).collect();
        for m in written {
            match self.kind(m) {
                NodeKind::Target => self.write_target(m),
                _ => self.comment(m),
            }
            self.done.push(m);
        }
    }

    /// Writes `text` as a title with the adornment of index `index`: underlined, or
    /// over- and underlined past the list of characters.
    fn write_title(&mut self, text: &str, index: usize) {
        let c = ADORNMENTS[index % ADORNMENTS.len()];
        let overline = index >= ADORNMENTS.len();
        let mut title = self.buffer();
        // Line breaks become spaces.
        for (k, piece) in text.split('\n').enumerate() {
            if k > 0 {
                title.push(' ');
            }
            title.push_str(piece);
        }
        if starts_construct(&title) {
            title.insert(0, '\\');
        }
        let mut rule = self.buffer();
        rule.extend(std::iter::repeat_n(c, width(&title).max(4)));
        self.block_start();
        if overline {
            self.line(&rule);
        }
        self.line(&title);
        self.line(&rule);
        self.block_end();
        self.recycle(title);
        self.recycle(rule);
    }

    /// The promoted document title.
    pub fn title(&mut self, id: NodeId) {
        self.write_moved(Some(id));
        self.targets_naming(id);
        let text = self.inline_line(id);
        self.write_title(&text, 0);
        self.recycle(text);
        if self.adornment_base < 2 {
            self.write_moved(None);
        }
    }

    /// The promoted document subtitle.
    pub fn subtitle(&mut self, id: NodeId) {
        self.write_moved(Some(id));
        self.targets_naming(id);
        let text = self.inline_line(id);
        self.write_title(&text, 1);
        self.recycle(text);
        self.write_moved(None);
    }

    /// Writes the targets the reader moved after a promoted title, which name it.
    fn targets_naming(&mut self, title: NodeId) {
        let targets: Vec<NodeId> = self
            .doc
            .children(ROOT)
            .filter(|&t| self.kind(t) == NodeKind::Target && self.names_earlier(t))
            .filter(|&t| self.doc.link_info(t).is_some_and(|l| l.resolved == title))
            .filter(|t| !self.done.contains(t))
            .collect();
        for t in targets {
            self.write_target(t);
        }
    }

    /// A section: its heading, then its body.
    pub fn section(&mut self, id: NodeId) {
        let level = usize::try_from(self.doc.node(id).value.max(1)).unwrap_or(1);
        let mut children = self.doc.children(id);
        let heading = children.next().filter(|&h| self.kind(h) == NodeKind::Heading);
        if let Some(h) = heading {
            let text = self.inline_line(h);
            self.write_title(&text, self.adornment_base + level - 1);
            self.recycle(text);
        } else {
            self.lossy(id, "a section without a heading", "its body alone");
        }
        let saved = self.prev_block;
        let saved_end = self.container_end;
        self.prev_block = None;
        self.container_end = self.doc.subtree_end(id).0;
        for child in self.doc.children(id).skip(usize::from(heading.is_some())) {
            self.node(child);
            self.prev_block = Some(child);
        }
        self.prev_block = saved;
        self.container_end = saved_end;
    }

    /// A heading outside a section (pure CommonMark): a section title of its level.
    pub fn heading(&mut self, id: NodeId) {
        let level = usize::try_from(self.doc.node(id).value.max(1)).unwrap_or(1);
        let text = self.inline_line(id);
        self.write_title(&text, self.adornment_base + level - 1);
        self.recycle(text);
    }
}
