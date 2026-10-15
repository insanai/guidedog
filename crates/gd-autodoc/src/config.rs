//! The `conf.toml` settings autodoc reads. Settings Sphinx takes as strings are enums
//! here, so a value autodoc does not know cannot reach it; hosts read the strings with
//! each enum's `parse`.

/// The `conf.toml` settings autodoc reads, with Sphinx's names; [`Default`] gives
/// Sphinx 9.1's defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `autoclass_content`: which docstrings a class shows.
    pub autoclass_content: ClassContent,
    /// `autodoc_class_signature`.
    pub autodoc_class_signature: ClassSignature,
    /// Options every directive gets unless it negates them.
    pub autodoc_default_options: Vec<DefaultOption>,
    /// Read a signature from the first line of a docstring.
    pub autodoc_docstring_signature: bool,
    /// Take a missing docstring from the same name along the MRO.
    pub autodoc_inherit_docstrings: bool,
    /// `autodoc_member_order`.
    pub autodoc_member_order: MemberOrder,
    /// Modules mocked at import (kept for the host; static analysis imports nothing).
    pub autodoc_mock_imports: Vec<String>,
    /// Show defaults as written.
    pub autodoc_preserve_defaults: bool,
    /// `autodoc_typehints`: where annotations show.
    pub autodoc_typehints: Typehints,
    /// `autodoc_typehints_description_target`.
    pub autodoc_typehints_description_target: DescriptionTarget,
    /// `autodoc_typehints_format`.
    pub autodoc_typehints_format: TypehintsFormat,
    /// Type aliases (kept for the host; not applied yet).
    pub autodoc_type_aliases: Vec<TypeAlias>,
    /// Read `# type:` comments as annotations.
    pub autodoc_use_type_comments: bool,
    /// Sphinx's `python_display_short_literal_types` (kept for the host).
    pub python_display_short_literal_types: bool,
    /// Escape backslashes in signatures.
    pub strip_signature_backslash: bool,
    /// The tab width docstrings are expanded with.
    pub tab_width: usize,
    /// Write the fields of `autodoc_typehints = "description"` into the text (true by
    /// default); Sphinx adds them to the parsed field lists instead, so comparisons
    /// with its generated text turn it off.
    pub typehint_fields: bool,
    /// `sphinx.ext.napoleon`'s settings when the extension is on: docstrings are read
    /// for Google and NumPy sections.
    pub napoleon: Option<crate::Napoleon>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            autoclass_content: ClassContent::Class,
            autodoc_class_signature: ClassSignature::Mixed,
            autodoc_default_options: Vec::new(),
            autodoc_docstring_signature: true,
            autodoc_inherit_docstrings: true,
            autodoc_member_order: MemberOrder::Alphabetical,
            autodoc_mock_imports: Vec::new(),
            autodoc_preserve_defaults: false,
            autodoc_typehints: Typehints::Signature,
            autodoc_typehints_description_target: DescriptionTarget::All,
            autodoc_typehints_format: TypehintsFormat::Short,
            autodoc_type_aliases: Vec::new(),
            autodoc_use_type_comments: true,
            python_display_short_literal_types: false,
            strip_signature_backslash: false,
            tab_width: 8,
            typehint_fields: true,
            napoleon: None,
        }
    }
}

/// Which docstrings a class shows (`autoclass_content`, `:class-doc-from:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClassContent {
    /// `"class"`: the class's own.
    #[default]
    Class,
    /// `"init"`: its constructor's.
    Init,
    /// `"both"`: the class's, then its constructor's.
    Both,
}

impl ClassContent {
    /// Reads Sphinx's name for the setting; `None` for an unknown one.
    pub fn parse(s: &str) -> Option<ClassContent> {
        match s {
            "class" => Some(ClassContent::Class),
            "init" => Some(ClassContent::Init),
            "both" => Some(ClassContent::Both),
            _ => None,
        }
    }
}

/// Where a class's signature shows (`autodoc_class_signature`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClassSignature {
    /// `"mixed"`: on the class.
    #[default]
    Mixed,
    /// `"separated"`: on its `__init__` and `__new__`, documented as members.
    Separated,
}

impl ClassSignature {
    /// Reads Sphinx's name for the setting; `None` for an unknown one.
    pub fn parse(s: &str) -> Option<ClassSignature> {
        match s {
            "mixed" => Some(ClassSignature::Mixed),
            "separated" => Some(ClassSignature::Separated),
            _ => None,
        }
    }
}

/// How members are ordered (`autodoc_member_order`, `:member-order:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemberOrder {
    /// `"alphabetical"`.
    #[default]
    Alphabetical,
    /// `"bysource"`: as `__all__` lists them, else as the source defines them.
    Bysource,
    /// `"groupwise"`: by kind, then alphabetically.
    Groupwise,
}

impl MemberOrder {
    /// Reads Sphinx's name for the setting; `None` for an unknown one.
    pub fn parse(s: &str) -> Option<MemberOrder> {
        match s {
            "alphabetical" => Some(MemberOrder::Alphabetical),
            "bysource" => Some(MemberOrder::Bysource),
            "groupwise" => Some(MemberOrder::Groupwise),
            _ => None,
        }
    }
}

/// Where annotations show (`autodoc_typehints`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Typehints {
    /// `"signature"`: in signatures.
    #[default]
    Signature,
    /// `"description"`: as `:type:` and `:rtype:` fields.
    Description,
    /// `"both"`.
    Both,
    /// `"none"`: nowhere.
    Hidden,
}

impl Typehints {
    /// Reads Sphinx's name for the setting; `None` for an unknown one.
    pub fn parse(s: &str) -> Option<Typehints> {
        match s {
            "signature" => Some(Typehints::Signature),
            "description" => Some(Typehints::Description),
            "both" => Some(Typehints::Both),
            "none" => Some(Typehints::Hidden),
            _ => None,
        }
    }

    /// Whether annotations show in signatures.
    pub(crate) fn in_signature(self) -> bool {
        matches!(self, Typehints::Signature | Typehints::Both)
    }

    /// Whether annotations are recorded for description fields.
    pub(crate) fn in_description(self) -> bool {
        matches!(self, Typehints::Description | Typehints::Both)
    }
}

/// Which parameters get description fields (`autodoc_typehints_description_target`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DescriptionTarget {
    /// `"all"`.
    #[default]
    All,
    /// `"documented"`: those the docstring describes.
    Documented,
    /// `"documented_params"`: those described, and the return type unless None.
    DocumentedParams,
}

impl DescriptionTarget {
    /// Reads Sphinx's name for the setting; `None` for an unknown one.
    pub fn parse(s: &str) -> Option<DescriptionTarget> {
        match s {
            "all" => Some(DescriptionTarget::All),
            "documented" => Some(DescriptionTarget::Documented),
            "documented_params" => Some(DescriptionTarget::DocumentedParams),
            _ => None,
        }
    }
}

/// How annotations name classes (`autodoc_typehints_format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypehintsFormat {
    /// `"short"`: `~module.Name`, shown as Name.
    #[default]
    Short,
    /// `"fully-qualified"`.
    FullyQualified,
}

impl TypehintsFormat {
    /// Reads Sphinx's name for the setting; `None` for an unknown one.
    pub fn parse(s: &str) -> Option<TypehintsFormat> {
        match s {
            "short" => Some(TypehintsFormat::Short),
            "fully-qualified" => Some(TypehintsFormat::FullyQualified),
            _ => None,
        }
    }
}

/// One `autodoc_default_options` entry: a value, or a flag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DefaultOption {
    /// The option, such as `"members"`.
    pub name: String,
    /// Its value; unused for a flag.
    pub value: String,
    /// The setting is `true`: the option is given without a value.
    pub flag: bool,
}

/// One `autodoc_type_aliases` entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeAlias {
    /// The alias as written in annotations.
    pub name: String,
    /// What it stands for.
    pub target: String,
}
