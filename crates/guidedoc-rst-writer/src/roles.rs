//! Role declarations. Inline markup that carries classes, a language, or a raw format
//! is written with a custom role, which the output must declare (`.. role::`) before
//! it is used; the declarations are written at the top of the document.

/// The roles every reader knows, which a declaration must not shadow.
const STANDARD_ROLES: &[&str] = &[
    "emphasis",
    "strong",
    "literal",
    "code",
    "math",
    "sub",
    "subscript",
    "sup",
    "superscript",
    "title-reference",
    "title",
    "t",
    "pep-reference",
    "PEP",
    "rfc-reference",
    "RFC",
    "abbreviation",
    "ab",
    "acronym",
    "ac",
    "raw",
    "ref",
    "doc",
    "any",
    "term",
    "numref",
    "keyword",
    "option",
    "envvar",
    "token",
    "download",
    "eq",
    "abbr",
    "index",
    "kbd",
    "guilabel",
    "menuselection",
    "command",
    "dfn",
    "file",
    "samp",
    "program",
    "mimetype",
    "newsgroup",
    "mailheader",
    "makevar",
    "manpage",
    "regexp",
];

/// The roles Sphinx gives a Role node of their own name; a document the Sphinx layer
/// read uses them without a declaration.
pub(crate) const SPHINX_ROLES: &[&str] = &[
    "kbd",
    "guilabel",
    "menuselection",
    "command",
    "dfn",
    "file",
    "samp",
    "program",
    "mimetype",
    "newsgroup",
    "mailheader",
    "makevar",
    "manpage",
    "regexp",
    "accelerator",
];

/// One declared role: `.. role:: name(base)` with options.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
    name: String,
    base: String,
    options: Vec<(String, String)>,
}

/// The roles a document needs declared, in the order of first use.
#[derive(Debug, Default)]
pub(crate) struct RoleTable {
    declared: Vec<Declaration>,
}

impl RoleTable {
    /// Returns the name of a role based on `base` (empty for none) with `options`,
    /// declaring it on first use. `wanted` is the preferred name.
    pub fn role(&mut self, wanted: &str, base: &str, options: &[(&str, &str)]) -> String {
        let options: Vec<(String, String)> =
            options.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        let found = self.declared.iter().find(|d| d.base == base && d.options == options);
        if let Some(d) = found.filter(|d| d.name.starts_with(&sanitize(wanted))) {
            return d.name.clone();
        }
        let stem = sanitize(wanted);
        let stem = if stem.is_empty() || STANDARD_ROLES.contains(&stem.as_str()) {
            format!("{base}-{stem}").trim_matches('-').to_owned()
        } else {
            stem
        };
        let mut name = stem.clone();
        let mut n = 2;
        while self.declared.iter().any(|d| d.name == name) {
            name = format!("{stem}-{n}");
            n += 1;
        }
        self.declared.push(Declaration { name: name.clone(), base: base.to_owned(), options });
        name
    }

    /// Declares a role named exactly `name` (a custom role the source defined), unless
    /// it is declared already.
    pub fn exact(&mut self, name: &str, options: &[(&str, &str)]) {
        if self.declared.iter().any(|d| d.name == name) {
            return;
        }
        let options = options.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        self.declared.push(Declaration { name: name.to_owned(), base: String::new(), options });
    }

    /// Writes the declarations, each followed by a blank line.
    pub fn write(&self, out: &mut String) {
        for d in &self.declared {
            out.push_str(".. role:: ");
            out.push_str(&d.name);
            if !d.base.is_empty() {
                out.push('(');
                out.push_str(&d.base);
                out.push(')');
            }
            out.push('\n');
            for (key, value) in &d.options {
                out.push_str("   :");
                out.push_str(key);
                out.push(':');
                if !value.is_empty() {
                    out.push(' ');
                    out.push_str(value);
                }
                out.push('\n');
            }
            out.push('\n');
        }
    }
}

/// Turns classes or a language into a role name: lowercase words joined by hyphens.
fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_declared_once_and_never_shadow() {
        let mut t = RoleTable::default();
        assert_eq!(t.role("red", "emphasis", &[("class", "red")]), "red");
        assert_eq!(t.role("red", "emphasis", &[("class", "red")]), "red");
        assert_eq!(t.role("red", "strong", &[("class", "red")]), "red-2");
        assert_eq!(t.role("code", "literal", &[("class", "code")]), "literal-code");
        let mut out = String::new();
        t.write(&mut out);
        assert!(out.starts_with(".. role:: red(emphasis)\n   :class: red\n\n"), "{out}");
    }
}
