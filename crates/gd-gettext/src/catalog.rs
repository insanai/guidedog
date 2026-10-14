//! Messages, catalogs, and the header entry.

use crate::index::Indexes;
use crate::plural::PluralRule;

/// A fuzzy message's earlier msgctxt, msgid, and msgid_plural (the `#|` comments msgmerge
/// writes), so a translator sees what changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Previous {
    /// The previous msgctxt; `Some("")` is an empty context, different from none.
    pub ctx: Option<String>,
    /// The previous msgid.
    pub id: String,
    /// The previous msgid_plural.
    pub id_plural: Option<String>,
}

/// One catalog entry.
///
/// `strs` holds the msgstr, or msgstr\[0..n) of a plural message; an empty string is an
/// untranslated form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    /// The msgctxt; `Some("")` (msgctxt "") is a context too, different from none.
    pub ctx: Option<String>,
    /// The msgid.
    pub id: String,
    /// The msgid_plural of a plural message.
    pub id_plural: Option<String>,
    /// The msgstr, or one string per plural form.
    pub strs: Vec<String>,
    /// `# ` translator comments.
    pub comments: Vec<String>,
    /// `#.` comments for translators, from the extractor.
    pub notes: Vec<String>,
    /// `#:` locations, `file:line` each.
    pub references: Vec<String>,
    /// `#,` flags other than fuzzy, e.g. `c-format`.
    pub flags: Vec<String>,
    /// The `fuzzy` flag.
    pub fuzzy: bool,
    /// `#~`: kept for reuse, never looked up.
    pub obsolete: bool,
    /// The `#|` previous msgid of a fuzzy message.
    pub previous: Option<Previous>,
    /// The 1-based line the entry starts on in its file; 0 when built in code.
    pub line: usize,
}

impl Message {
    /// Returns a message with this msgid and one empty (untranslated) msgstr.
    pub fn new(id: &str) -> Message {
        Message { id: id.to_string(), strs: vec![String::new()], ..Message::default() }
    }

    /// Tells whether every form has a translation (and there is at least one).
    pub fn is_translated(&self) -> bool {
        !self.strs.is_empty() && self.strs.iter().all(|s| !s.is_empty())
    }

    /// Tells whether this is the header entry: msgid "" without a context, not obsolete.
    pub fn is_header(&self) -> bool {
        self.id.is_empty() && self.ctx.is_none() && !self.obsolete
    }

    /// Tells whether the message has a msgid_plural.
    pub fn is_plural(&self) -> bool {
        self.id_plural.is_some()
    }

    /// Tells whether the message carries a `#,` flag; `"fuzzy"` is [`Message::fuzzy`].
    pub fn has_flag(&self, flag: &str) -> bool {
        if flag == "fuzzy" {
            return self.fuzzy;
        }
        self.flags.iter().any(|f| f == flag)
    }

    /// Tells whether two messages have the same context and msgid.
    pub fn same_key(&self, other: &Message) -> bool {
        self.ctx == other.ctx && self.id == other.id
    }
}

/// The lookup key of a message as the MO format writes it: msgctxt `\x04` msgid.
pub(crate) fn message_key(m: &Message) -> String {
    match &m.ctx {
        Some(ctx) => format!("{ctx}\x04{}", m.id),
        None => m.id.clone(),
    }
}

/// One message catalog: a PO, POT, or MO file's messages.
///
/// The catalog owns everything it holds. Lookups (the [`crate::Lookup`] trait) use an
/// index built on first use; [`Catalog::add_message`], [`Catalog::set_header_field`] and
/// [`crate::sort_by_reference`] drop it. After editing `messages` or `header` directly,
/// call [`Catalog::build_index`] before the next lookup.
#[derive(Debug, Clone)]
pub struct Catalog {
    /// The file name, or the name given to a parser.
    pub name: String,
    /// The msgid "" entry; its `strs[0]` holds the header fields.
    pub header: Option<Message>,
    /// The messages in file order; obsolete ones included.
    pub messages: Vec<Message>,
    /// The compiled Plural-Forms rule; `n != 1` without a header.
    pub plural: PluralRule,
    /// Look fuzzy translations up too (Sphinx's `gettext_allow_fuzzy_translations`).
    pub allow_fuzzy: bool,
    /// The lookup indexes: without fuzzy messages, and with them.
    pub(crate) index: Indexes,
}

impl Default for Catalog {
    fn default() -> Catalog {
        Catalog::new("")
    }
}

impl Catalog {
    /// Returns an empty catalog named `name` with the germanic plural rule (`n != 1`).
    pub fn new(name: &str) -> Catalog {
        Catalog {
            name: name.to_string(),
            header: None,
            messages: Vec::new(),
            plural: PluralRule::default(),
            allow_fuzzy: false,
            index: Indexes::default(),
        }
    }

    /// Appends a message, which the catalog owns from now on, and returns a reference to
    /// it. The lookup index is rebuilt on the next lookup.
    pub fn add_message(&mut self, m: Message) -> &mut Message {
        self.drop_index();
        self.messages.push(m);
        let last = self.messages.len() - 1;
        &mut self.messages[last]
    }

    /// Returns the number of messages, obsolete ones included, without the header.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Forgets the lookup indexes, so the next lookup builds them again.
    pub(crate) fn drop_index(&mut self) {
        self.index = Indexes::default();
    }

    /// Returns the value of a header field (`"Name: value"` line), compared
    /// case-insensitively, borrowed from the header.
    pub fn header_field(&self, name: &str) -> Option<&str> {
        header_field_in(self.header.as_ref()?.strs.first()?, name)
    }

    /// Replaces a header field, or appends it, keeping the others in order. Setting
    /// `Plural-Forms` to a valid rule also replaces [`Catalog::plural`].
    pub fn set_header_field(&mut self, name: &str, value: &str) {
        self.drop_index();
        let header = self.header.get_or_insert_with(Message::default);
        if header.strs.is_empty() {
            header.strs.push(String::new());
        }
        let mut out = String::new();
        let mut found = false;
        for line in header.strs[0].lines() {
            let matches =
                line.split_once(':').is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case(name));
            if matches {
                if !found {
                    out.push_str(&format!("{name}: {value}\n"));
                }
                found = true;
            } else if !line.is_empty() {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !found {
            out.push_str(&format!("{name}: {value}\n"));
        }
        header.strs[0] = out;
        // A rule that does not compile leaves the current one in place.
        if name.eq_ignore_ascii_case("Plural-Forms")
            && let Ok(rule) = crate::parse_plural_forms(value)
        {
            self.plural = rule;
        }
    }

    /// Returns the charset the header's Content-Type names, or `""` without one.
    pub fn charset(&self) -> &str {
        self.header.as_ref().and_then(|h| h.strs.first()).map_or("", |text| charset_in(text))
    }
}

/// Returns the value of field `name` in header text, compared case-insensitively.
pub(crate) fn header_field_in<'h>(header: &'h str, name: &str) -> Option<&'h str> {
    header.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// Returns the charset the Content-Type field of header text names, or `""`.
pub(crate) fn charset_in(header: &str) -> &str {
    let Some(ct) = header_field_in(header, "Content-Type") else { return "" };
    let Some(at) = ct.find("charset=") else { return "" };
    let rest = &ct[at + "charset=".len()..];
    let end = rest.find([';', ' ']).unwrap_or(rest.len());
    &rest[..end]
}

/// Tells whether a charset name is UTF-8 or a subset of it. POT templates say `CHARSET`
/// until a translator fills it in.
pub(crate) fn is_utf8_charset(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "" | "utf-8" | "utf8" | "charset" | "ascii" | "us-ascii"
    )
}
