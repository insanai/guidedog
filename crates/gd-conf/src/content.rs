//! The content rules: whether a build keeps raw content (`.. raw:: html`, raw Typst)
//! and unsafe URLs (`javascript:` and the like). conf.toml states them with
//! `raw_content` and `unsafe_urls`; a build may decide them instead (GDS 0018).
//!
//! The rules are separate from what a build may read (its confinement): raw content
//! harms the reader of the output, not the machine that builds it.

/// What a build does with one kind of content a reader might not trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentRule {
    /// Keep it, as Sphinx does.
    #[default]
    Allow,
    /// Leave it out, with a warning.
    Omit,
}

impl ContentRule {
    /// The rule a conf.toml value names: `"allow"` or `"omit"`; `None` for any other.
    pub fn from_name(name: &str) -> Option<ContentRule> {
        match name {
            "allow" => Some(ContentRule::Allow),
            "omit" => Some(ContentRule::Omit),
            _ => None,
        }
    }

    /// The name conf.toml writes: `"allow"` or `"omit"`.
    pub fn name(self) -> &'static str {
        match self {
            ContentRule::Allow => "allow",
            ContentRule::Omit => "omit",
        }
    }
}

/// The two content rules. A rule that is `None` is not stated: conf.toml leaves it
/// unset, or a build leaves it to the project. Fill it with struct-update syntax:
/// `ContentRules { raw: Some(ContentRule::Omit), ..Default::default() }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ContentRules {
    /// `raw_content`: raw HTML, Typst and LaTeX.
    pub raw: Option<ContentRule>,
    /// `unsafe_urls`: `javascript:`, `vbscript:` and non-image `data:` URLs.
    pub unsafe_urls: Option<ContentRule>,
}

impl ContentRules {
    /// Both rules stated as `omit`: what an `--untrusted` build uses.
    pub const OMIT: ContentRules =
        ContentRules { raw: Some(ContentRule::Omit), unsafe_urls: Some(ContentRule::Omit) };

    /// Both rules stated as `allow`: Sphinx's behaviour.
    pub const ALLOW: ContentRules =
        ContentRules { raw: Some(ContentRule::Allow), unsafe_urls: Some(ContentRule::Allow) };

    /// Each rule of `self`, or `fallback`'s where `self` states none.
    pub fn or(self, fallback: ContentRules) -> ContentRules {
        ContentRules {
            raw: self.raw.or(fallback.raw),
            unsafe_urls: self.unsafe_urls.or(fallback.unsafe_urls),
        }
    }

    /// The rule for raw content: `allow` when none is stated.
    pub fn raw_rule(self) -> ContentRule {
        self.raw.unwrap_or_default()
    }

    /// The rule for unsafe URLs: `allow` when none is stated.
    pub fn unsafe_url_rule(self) -> ContentRule {
        self.unsafe_urls.unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_named_as_conf_toml_writes_them() {
        for rule in [ContentRule::Allow, ContentRule::Omit] {
            assert_eq!(ContentRule::from_name(rule.name()), Some(rule));
        }
        assert_eq!(ContentRule::from_name("refuse"), None);
    }

    #[test]
    fn unstated_rules_fall_back_and_then_allow() {
        let project = ContentRules { raw: Some(ContentRule::Omit), ..Default::default() };
        let build = ContentRules { unsafe_urls: Some(ContentRule::Omit), ..Default::default() };
        assert_eq!(build.or(project), ContentRules::OMIT);
        assert_eq!(ContentRules::default().raw_rule(), ContentRule::Allow);
        assert_eq!(project.unsafe_url_rule(), ContentRule::Allow);
        assert_eq!(ContentRules::ALLOW.or(ContentRules::OMIT), ContentRules::ALLOW);
    }
}
