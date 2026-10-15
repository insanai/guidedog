//! The sections napoleon knows (`Args:`, `Returns`, `See Also`, ...) and how each one
//! becomes reStructuredText: fields autodoc renders, rubrics, or admonitions.

use super::docstring::{
    Field, Parser, any, dedent, format_admonition, format_block, format_block_padded,
    format_field, indent, partition_on_colon, strip_empty,
};
use super::patterns::{is_xref, match_name};
use super::{DocstringStyle, NapoleonSection};

/// What a section header starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Parameters,
    Attributes,
    Admonition(&'static str),
    Examples,
    KeywordArguments,
    Methods,
    Notes,
    OtherParameters,
    Receives,
    Returns,
    Raises,
    References,
    SeeAlso,
    Warns,
    Yields,
    /// A custom section shown as a rubric and its text.
    CustomGeneric,
    /// A custom section of fields, like `Parameters` without `:param:` fields.
    CustomParams,
    /// A custom section like `Returns`.
    CustomReturns,
}

/// The sections every docstring knows, by lower-case header.
const SECTIONS: &[(&str, Section)] = &[
    ("args", Section::Parameters),
    ("arguments", Section::Parameters),
    ("attention", Section::Admonition("attention")),
    ("attributes", Section::Attributes),
    ("caution", Section::Admonition("caution")),
    ("danger", Section::Admonition("danger")),
    ("error", Section::Admonition("error")),
    ("example", Section::Examples),
    ("examples", Section::Examples),
    ("hint", Section::Admonition("hint")),
    ("important", Section::Admonition("important")),
    ("keyword args", Section::KeywordArguments),
    ("keyword arguments", Section::KeywordArguments),
    ("methods", Section::Methods),
    ("note", Section::Admonition("note")),
    ("notes", Section::Notes),
    ("other parameters", Section::OtherParameters),
    ("parameters", Section::Parameters),
    ("receive", Section::Receives),
    ("receives", Section::Receives),
    ("return", Section::Returns),
    ("returns", Section::Returns),
    ("raise", Section::Raises),
    ("raises", Section::Raises),
    ("references", Section::References),
    ("see also", Section::SeeAlso),
    ("tip", Section::Admonition("tip")),
    ("todo", Section::Admonition("todo")),
    ("warning", Section::Admonition("warning")),
    ("warnings", Section::Admonition("warning")),
    ("warn", Section::Warns),
    ("warns", Section::Warns),
    ("yield", Section::Yields),
    ("yields", Section::Yields),
];

/// `_load_custom_sections`: the built-in sections with `napoleon_custom_sections`
/// added, and the custom entries whose `like` names no section.
pub(crate) fn section_table(custom: &[NapoleonSection]) -> (Vec<(String, Section)>, Vec<String>) {
    let mut table: Vec<(String, Section)> =
        SECTIONS.iter().map(|(n, s)| ((*n).to_owned(), *s)).collect();
    let mut unknown = Vec::new();
    for entry in custom {
        let section = match entry.like.as_str() {
            "" => Section::CustomGeneric,
            "params_style" => Section::CustomParams,
            "returns_style" => Section::CustomReturns,
            like => {
                let lower = like.to_lowercase();
                match table.iter().find(|(n, _)| *n == lower) {
                    Some((_, s)) => *s,
                    None => {
                        unknown.push(entry.like.clone());
                        Section::CustomGeneric
                    }
                }
            }
        };
        let name = entry.name.to_lowercase();
        match table.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = section,
            None => table.push((name, section)),
        }
    }
    (table, unknown)
}

impl Parser<'_> {
    /// The body of a section whose header was `section`.
    pub(crate) fn parse_section_body(&mut self, kind: Section, section: &str) -> Vec<String> {
        self.strict_names = matches!(
            kind,
            Section::Parameters
                | Section::OtherParameters
                | Section::Receives
                | Section::KeywordArguments
                | Section::Attributes
        );
        let lines = self.section_body(kind, section);
        self.strict_names = false;
        lines
    }

    fn section_body(&mut self, kind: Section, section: &str) -> Vec<String> {
        let s = self.shared.settings;
        match kind {
            Section::Parameters => self.parameters("Parameters"),
            Section::OtherParameters => self.parameters("Other Parameters"),
            Section::Receives => self.parameters("Receives"),
            Section::Attributes => self.attributes(),
            Section::Admonition(name) => {
                let lines = self.consume_to_next_section();
                format_admonition(name, lines)
            }
            Section::Examples => {
                let label = match section.to_lowercase().as_str() {
                    "example" => "Example".to_owned(),
                    "examples" => "Examples".to_owned(),
                    _ => section.to_owned(),
                };
                self.generic(&label, s.use_admonition_for_examples)
            }
            Section::KeywordArguments => self.keyword_arguments(),
            Section::Methods => self.methods(),
            Section::Notes => self.generic("Notes", s.use_admonition_for_notes),
            Section::References => self.generic("References", s.use_admonition_for_references),
            Section::Returns => self.returns(),
            Section::Raises => self.raises(),
            Section::SeeAlso => self.see_also(),
            Section::Warns => {
                let fields = self.consume_fields(true, false, false);
                format_fields("Warns", fields)
            }
            Section::Yields => {
                let fields = self.consume_returns_section(true);
                format_fields("Yields", fields)
            }
            Section::CustomGeneric => self.generic(section, false),
            Section::CustomParams => {
                let fields = self.consume_fields(true, false, false);
                format_fields(section, fields)
            }
            Section::CustomReturns => {
                let fields = self.consume_returns_section(true);
                format_fields(section, fields)
            }
        }
    }

    /// `_parse_parameters_section` and its kin: `:param:` fields, or a field list
    /// under `label` without `napoleon_use_param`.
    fn parameters(&mut self, label: &str) -> Vec<String> {
        if self.shared.settings.use_param {
            let fields = self.consume_fields(true, false, true);
            return format_docutils_params(fields, "param", "type");
        }
        let fields = self.consume_fields(true, false, false);
        format_fields(label, fields)
    }

    /// `_parse_keyword_arguments_section`.
    fn keyword_arguments(&mut self) -> Vec<String> {
        let fields = self.consume_fields(true, false, false);
        if self.shared.settings.use_keyword {
            return format_docutils_params(fields, "keyword", "kwtype");
        }
        format_fields("Keyword Arguments", fields)
    }

    /// `_parse_generic_section`: a rubric (or an admonition) and the text.
    fn generic(&mut self, section: &str, use_admonition: bool) -> Vec<String> {
        let lines = dedent(strip_empty(self.consume_to_next_section()));
        let (header, lines) = if use_admonition {
            (format!(".. admonition:: {section}"), indent(&lines, 3))
        } else {
            (format!(".. rubric:: {section}"), lines)
        };
        let mut out = vec![header, String::new()];
        if !lines.is_empty() {
            out.extend(lines);
            out.push(String::new());
        }
        out
    }

    /// `_parse_attributes_section`: `.. attribute::` directives, or `:ivar:` fields.
    fn attributes(&mut self) -> Vec<String> {
        let use_ivar = self.shared.settings.use_ivar;
        let mut lines = Vec::new();
        for f in self.consume_fields(true, false, false) {
            let ty = if f.ty.is_empty() { self.lookup_annotation(&f.name) } else { f.ty };
            if use_ivar {
                lines.extend(format_block(&format!(":ivar {}: ", f.name), &f.desc));
                if !ty.is_empty() {
                    lines.push(format!(":vartype {}: {ty}", f.name));
                }
                continue;
            }
            lines.push(format!(".. attribute:: {}", f.name));
            if self.no_index {
                lines.push("   :no-index:".to_owned());
            }
            lines.push(String::new());
            lines.extend(indent(&format_field("", "", f.desc), 3));
            if !ty.is_empty() {
                lines.push(String::new());
                lines.push(format!("   :type: {ty}"));
            }
            lines.push(String::new());
        }
        if use_ivar {
            lines.push(String::new());
        }
        lines
    }

    /// `_parse_methods_section`: a `.. method::` directive per entry.
    fn methods(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        for f in self.consume_fields(false, false, false) {
            lines.push(format!(".. method:: {}", f.name));
            if self.no_index {
                lines.push("   :no-index:".to_owned());
            }
            if !f.desc.is_empty() {
                lines.push(String::new());
                lines.extend(indent(&f.desc, 3));
            }
            lines.push(String::new());
        }
        lines
    }

    /// `_consume_returns_section`: Google's one `type: description` field, NumPy's
    /// fields whose names are types.
    pub(crate) fn consume_returns_section(&mut self, preprocess: bool) -> Vec<Field> {
        if self.style == DocstringStyle::Numpy {
            return self.consume_fields(true, true, false);
        }
        let at = self.position();
        let lines = dedent(self.consume_to_next_section());
        let Some(first) = lines.first() else { return Vec::new() };
        let (before, colon, after) = partition_on_colon(first);
        let (mut ty, desc) = if colon.is_empty() {
            (String::new(), lines.clone())
        } else if after.is_empty() {
            (before, lines[1..].to_vec())
        } else {
            let mut desc = vec![after];
            desc.extend_from_slice(&lines[1..]);
            (before, desc)
        };
        if !ty.is_empty() && preprocess && self.shared.settings.preprocess_types {
            ty = self.preprocess_type(&ty, at);
        }
        vec![Field { name: String::new(), ty, desc: self.nested(desc) }]
    }

    /// `_parse_returns_section`: `:returns:` and `:rtype:` fields.
    fn returns(&mut self) -> Vec<String> {
        let fields = self.consume_returns_section(false);
        let multi = fields.len() > 1;
        let use_rtype = !multi && self.shared.settings.use_rtype;
        let mut lines: Vec<String> = Vec::new();
        for f in fields {
            let field = format_field(&f.name, if use_rtype { "" } else { &f.ty }, f.desc);
            if multi {
                let prefix = if lines.is_empty() { ":returns: * " } else { "          * " };
                lines.extend(format_block(prefix, &field));
                continue;
            }
            if any(&field) {
                lines.extend(format_block(":returns: ", &field));
            }
            if !f.ty.is_empty() && use_rtype {
                lines.push(format!(":rtype: {}", f.ty));
                lines.push(String::new());
            }
        }
        if lines.last().is_some_and(|l| !l.is_empty()) {
            lines.push(String::new());
        }
        lines
    }

    /// `_parse_raises_section`: a `:raises:` field per exception.
    fn raises(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        for f in self.consume_fields(false, true, false) {
            let mut ty = f.ty;
            match match_name(&ty) {
                Some(m) if m.ticked => ty = m.name.to_owned(),
                _ if is_xref(&ty) => {
                    let pos = ty.find('`').map_or(0, |p| p + 1);
                    let mut inner = ty[pos..].to_owned();
                    inner.pop();
                    ty = inner;
                }
                _ => {}
            }
            let ty = if ty.is_empty() { String::new() } else { format!(" {ty}") };
            let desc = strip_empty(f.desc);
            if !any(&desc) {
                lines.push(format!(":raises{ty}:"));
                continue;
            }
            lines.push(format!(":raises{ty}: {}", desc[0]));
            lines.extend(desc[1..].iter().map(|d| format!("    {d}")));
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }

    /// `_parse_see_also_section`: a `seealso` admonition; NumPy's entries become
    /// links.
    fn see_also(&mut self) -> Vec<String> {
        let lines = self.consume_to_next_section();
        if self.style == DocstringStyle::Numpy
            && let Some(out) = self.numpy_see_also(&lines)
        {
            return out;
        }
        format_admonition("seealso", lines)
    }
}

/// `_format_docutils_params`: `:param x:` and `:type x:` fields.
fn format_docutils_params(fields: Vec<Field>, field_role: &str, type_role: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for f in fields {
        let desc = strip_empty(f.desc);
        if any(&desc) {
            let desc = super::docstring::fix_field_desc(desc);
            lines.extend(format_block(&format!(":{field_role} {}: ", f.name), &desc));
        } else {
            lines.push(format!(":{field_role} {}:", f.name));
        }
        if !f.ty.is_empty() {
            lines.push(format!(":{type_role} {}: {}", f.name, f.ty));
        }
    }
    lines.push(String::new());
    lines
}

/// `_format_fields`: a field list `:label:` of `**name** (*type*) -- description`
/// items, bulleted when there are several.
pub(crate) fn format_fields(label: &str, fields: Vec<Field>) -> Vec<String> {
    let field_type = format!(":{}:", label.trim());
    let padding = " ".repeat(field_type.chars().count());
    let multi = fields.len() > 1;
    let mut lines: Vec<String> = Vec::new();
    for f in fields {
        let field = format_field(&f.name, &f.ty, f.desc);
        let prefix = match (multi, lines.is_empty()) {
            (true, false) => format!("{padding} * "),
            (true, true) => format!("{field_type} * "),
            (false, _) => format!("{field_type} "),
        };
        let pad = " ".repeat(prefix.chars().count());
        lines.extend(format_block_padded(&prefix, &field, &pad));
    }
    if lines.last().is_some_and(|l| !l.is_empty()) {
        lines.push(String::new());
    }
    lines
}
