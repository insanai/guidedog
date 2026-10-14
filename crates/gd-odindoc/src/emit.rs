//! The reST of the odin domain's generating directives, as sphinx.ext.autodoc writes a
//! module's or class's: `odin:autopackage` writes the package's `odin:package`
//! directive, its doc comment, and with `:members:` its declarations;
//! `odin:autoproc`, `odin:autoprocgroup`, `odin:autotype`, `odin:autoconst`, and
//! `odin:autovar` write one declaration (every platform variant of its name), with its
//! fields or enumerators when `:members:` is given.

use crate::comments::doc_rest;
use crate::{
    Config, Decl, DirectiveOption, Kind, Member, MemberKind, MemberOrder, Message, Output,
    Package, Private, Project, Request, Severity,
};

/// The options a directive was given, over the configured defaults.
#[derive(Debug, Default)]
struct Options {
    /// `:members:`: `None` when absent, else the names (empty for all).
    members: Option<String>,
    undoc: bool,
    private: bool,
    exclude: Vec<String>,
    order: MemberOrder,
    no_index: bool,
}

const OPTION_NAMES: [&str; 7] = [
    "members",
    "undoc-members",
    "private-members",
    "exclude-members",
    "member-order",
    "no-index",
    "noindex",
];

struct Emitter<'r> {
    options: Options,
    request: &'r Request,
    out: Output,
}

/// Runs one autodoc directive (`request.directive`, "odin:autopackage" and the others,
/// with its argument and options over `config`'s defaults) and returns the
/// reStructuredText lines it generates, with any messages: an unknown directive,
/// option, package, or name is an Error message and no lines. Packages are loaded
/// through `project` as needed.
pub fn document(project: &mut Project, config: &Config, request: &Request) -> Output {
    let mut e = Emitter { options: Options::default(), request, out: Output::default() };
    let Some(kinds) = directive_kinds(&request.directive) else {
        e.report(
            Severity::Error,
            "odin.autodoc.directive",
            "UNKNOWN ODIN AUTODOC DIRECTIVE",
            format!("{} is not a directive that generates Odin documentation.", request.directive),
            "Use odin:autopackage, odin:autoproc, odin:autoprocgroup, odin:autotype, \
             odin:autoconst, or odin:autovar.",
        );
        return e.out;
    };
    let Some(options) = e.read_options(config) else {
        return e.out;
    };
    e.options = options;
    if request.directive.ends_with("autopackage") {
        e.emit_package(project);
    } else {
        e.emit_object(project, kinds);
    }
    e.out
}

/// The kinds a directive documents (none for `autopackage`, which documents a package).
fn directive_kinds(directive: &str) -> Option<&'static [Kind]> {
    use Kind::*;
    let name = directive.strip_prefix("odin:").unwrap_or(directive);
    let kinds: &'static [Kind] = match name {
        "autopackage" => &[],
        "autoproc" | "autoprocedure" => &[Proc, ProcGroup],
        "autoprocgroup" => &[ProcGroup],
        "autotype" => &[Struct, Union, Enum, BitSet, BitField, Type],
        "autoconst" => &[Const],
        "autovar" => &[Var],
        _ => return None,
    };
    Some(kinds)
}

/// Names separated by commas, without blanks.
fn split_names(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).collect()
}

/// The groups `:member-order: groupwise` shows members in.
const GROUPS: [(&str, &[Kind]); 6] = [
    ("Types", &[Kind::Struct, Kind::Union, Kind::Enum, Kind::BitSet, Kind::BitField, Kind::Type]),
    ("Procedures", &[Kind::Proc]),
    ("Procedure groups", &[Kind::ProcGroup]),
    ("Constants", &[Kind::Const]),
    ("Variables", &[Kind::Var]),
    ("Foreign imports", &[Kind::ForeignImport]),
];

impl Emitter<'_> {
    fn report(&mut self, severity: Severity, code: &str, title: &str, text: String, hint: &str) {
        self.out.messages.push(Message {
            severity,
            code: code.to_string(),
            title: title.to_string(),
            text,
            hint: hint.to_string(),
        });
    }

    fn read_options(&mut self, config: &Config) -> Option<Options> {
        let mut o = Options { order: config.member_order, ..Options::default() };
        let given = &self.request.options;
        let defaults =
            config.default_options.iter().filter(|d| !given.iter().any(|g| g.name == d.name));
        let all: Vec<&DirectiveOption> = defaults.chain(given.iter()).collect();
        for opt in all {
            if !OPTION_NAMES.contains(&opt.name.as_str()) {
                self.report(
                    Severity::Error,
                    "odin.autodoc.option",
                    "UNKNOWN ODIN AUTODOC OPTION",
                    format!(":{}: is not an option of {}.", opt.name, self.request.directive),
                    "Use :members:, :undoc-members:, :private-members:, :exclude-members:, \
                     :member-order:, or :no-index:.",
                );
                return None;
            }
            let value = opt.value.trim();
            match opt.name.as_str() {
                "members" => o.members = Some(value.to_string()),
                "undoc-members" => o.undoc = true,
                "private-members" => o.private = true,
                "exclude-members" => o.exclude = split_names(value),
                "member-order" => o.order = self.member_order(value)?,
                _ => o.no_index = true,
            }
        }
        Some(o)
    }

    fn member_order(&mut self, value: &str) -> Option<MemberOrder> {
        match value {
            "source" | "bysource" => return Some(MemberOrder::Source),
            "alphabetical" => return Some(MemberOrder::Alphabetical),
            "groupwise" => return Some(MemberOrder::Groupwise),
            _ => {}
        }
        self.report(
            Severity::Error,
            "odin.autodoc.order",
            "UNKNOWN MEMBER ORDER",
            format!(
                ":member-order: is {value}, which is not an order Odin members can be shown in."
            ),
            "Use source, alphabetical, or groupwise.",
        );
        None
    }

    /// Loads a package and reports what kept parts of it from being read.
    fn load_package<'p>(&mut self, project: &'p mut Project, id: &str) -> Option<&'p Package> {
        let Some(pkg) = project.load_package(id) else {
            self.report(
                Severity::Error,
                "odin.autodoc.package",
                "ODIN PACKAGE NOT FOUND",
                format!(
                    "No folder below the configured Odin source folders holds the package {id}."
                ),
                "Name the package by its path below one of odin_autoapi_dirs, such as \
                 readers/sphinx, or by collection, such as core:fmt.",
            );
            return None;
        };
        if pkg.errors > 0 {
            self.report(
                Severity::Warning,
                "odin.autodoc.syntax",
                "ODIN SOURCE DOES NOT PARSE",
                format!("The package {id} has syntax errors; what could be read is documented."),
                "Check the package with odin check.",
            );
        }
        for path in &pkg.too_deep {
            self.report(
                Severity::Warning,
                "odin.autodoc.nesting",
                "ODIN SOURCE NESTS TOO DEEPLY",
                format!(
                    "The file {path} of the package {id} nests too deeply to parse safely, so \
                     it is left out; the rest is documented."
                ),
                "Split the deeply nested expression, type or else-if chain.",
            );
        }
        for path in &pkg.too_large {
            self.report(
                Severity::Warning,
                "odin.autodoc.memory",
                "ODIN SOURCE TOO LARGE",
                format!(
                    "The file {path} of the package {id} is larger than the parse limit, so it \
                     is left out; the rest is documented."
                ),
                "Split the file into smaller Odin files, or raise the host's \
                 Limits::max_file_bytes.",
            );
        }
        Some(pkg)
    }

    fn emit_package(&mut self, project: &mut Project) {
        let mut id = self.request.argument.trim();
        if id.is_empty() {
            id = &self.request.current;
        }
        let Some(pkg) = self.load_package(project, id) else {
            return;
        };
        let lines = &mut self.out.lines;
        lines.push(format!(".. odin:package:: {}", pkg.id));
        if !pkg.synopsis.is_empty() {
            lines.push(format!("   :synopsis: {}", pkg.synopsis));
        }
        if !pkg.platform.is_empty() {
            lines.push(format!("   :platform: {}", pkg.platform));
        }
        if !pkg.imports.is_empty() {
            let parts: Vec<String> =
                pkg.imports.iter().map(|im| format!("{}={}", im.alias, im.id)).collect();
            lines.push(format!("   :imports: {}", parts.join(" ")));
        }
        let hidden = private_names(&self.options, pkg);
        if !hidden.is_empty() {
            self.out.lines.push(format!("   :private: {hidden}"));
        }
        if self.options.no_index {
            self.out.lines.push("   :no-index:".to_string());
        }
        self.out.lines.push(String::new());
        self.out.lines.extend(doc_rest(&pkg.doc));
        if !pkg.doc.is_empty() {
            self.out.lines.push(String::new());
        }
        if self.options.members.is_some() {
            self.emit_members(pkg);
        }
    }

    /// Writes the package's declarations that the options choose, in their order.
    fn emit_members(&mut self, pkg: &Package) {
        let mut chosen = self.choose(pkg);
        if self.options.order != MemberOrder::Source {
            chosen.sort_by(|a, b| a.name.cmp(&b.name));
        }
        if self.options.order != MemberOrder::Groupwise {
            for d in chosen {
                self.emit_decl(pkg, d, "", false);
            }
            return;
        }
        for (title, kinds) in GROUPS {
            let mut first = true;
            for d in chosen.iter().filter(|d| kinds.contains(&d.kind)) {
                if first {
                    self.out.lines.push(format!(".. rubric:: {title}"));
                    self.out.lines.push(String::new());
                    first = false;
                }
                self.emit_decl(pkg, d, "", false);
            }
        }
    }

    /// The declarations `:members:` names, or with an empty `:members:` every one that
    /// is public (or private, with `:private-members:`) and documented (or not, with
    /// `:undoc-members:`), without those `:exclude-members:` names.
    fn choose<'p>(&mut self, pkg: &'p Package) -> Vec<&'p Decl> {
        let names = split_names(self.options.members.as_deref().unwrap_or(""));
        if !names.is_empty() {
            let mut out = Vec::new();
            for name in names {
                let before = out.len();
                out.extend(pkg.decls.iter().filter(|d| d.name == name));
                if out.len() == before {
                    self.not_found(&pkg.id, &name);
                }
            }
            return out;
        }
        let o = &self.options;
        pkg.decls
            .iter()
            .filter(|d| !o.exclude.contains(&d.name))
            .filter(|d| d.private == Private::Public || o.private)
            .filter(|d| !d.docs.trim().is_empty() || o.undoc)
            .collect()
    }

    fn not_found(&mut self, pkg: &str, name: &str) {
        self.report(
            Severity::Warning,
            "odin.autodoc.object",
            "ODIN DECLARATION NOT FOUND",
            format!("The package {pkg} declares nothing named {name}."),
            "Check the name's spelling, and that its file builds for some target.",
        );
    }

    /// Writes one declaration of a package: argument "name" in the current package, or
    /// "package.name".
    fn emit_object(&mut self, project: &mut Project, kinds: &[Kind]) {
        let arg = self.request.argument.trim();
        let (mut id, mut name) = (self.request.current.as_str(), arg);
        let start = arg.rfind([':', '/']).map_or(0, |k| k + 1);
        if let Some(dot) = arg[start..].rfind('.') {
            (id, name) = (&arg[..start + dot], &arg[start + dot + 1..]);
        }
        let Some(pkg) = self.load_package(project, id) else {
            return;
        };
        let mut found = false;
        for d in pkg.decls.iter().filter(|d| d.name == name) {
            found = true;
            if !kinds.contains(&d.kind) {
                self.report(
                    Severity::Warning,
                    "odin.autodoc.kind",
                    "ODIN DECLARATION OF ANOTHER KIND",
                    format!(
                        "{}.{name} is a {}, which {} does not document.",
                        pkg.id,
                        d.kind.objtype(),
                        self.request.directive
                    ),
                    &format!("Use odin:auto{} instead.", auto_name(d.kind)),
                );
            }
            self.emit_decl(pkg, d, "", true);
        }
        if !found {
            self.not_found(&pkg.id, name);
        }
    }

    /// Writes a declaration's directive at an indentation, with its members when they
    /// are asked for.
    fn emit_decl(&mut self, pkg: &Package, d: &Decl, indent: &str, single: bool) {
        let mut head = d.attributes.join(" ");
        if !head.is_empty() {
            head.push(' ');
        }
        head.push_str(&d.signature);
        let lines = &mut self.out.lines;
        lines.push(format!("{indent}.. odin:{}:: {head}", d.kind.objtype()));
        let more = format!("{indent}   ");
        if single && pkg.id != self.request.current {
            lines.push(format!("{more}:package: {}", pkg.id));
        }
        if !d.availability.is_empty() {
            lines.push(format!("{more}:availability: {}", d.availability));
        }
        if !d.foreign_lib.is_empty() {
            lines.push(format!("{more}:foreign: {}", d.foreign_lib));
        }
        if self.options.no_index || d.variant > 0 {
            lines.push(format!("{more}:no-index:"));
        }
        lines.push(String::new());
        self.push_docs(&d.docs, &more);
        if self.options.members.is_none() && single {
            return;
        }
        for m in &d.members {
            if !m.docs.trim().is_empty() || self.options.undoc {
                self.emit_member(d, m, &more);
            }
        }
    }

    /// A doc comment's reST, indented, and a blank line after it.
    fn push_docs(&mut self, docs: &str, indent: &str) {
        for l in doc_rest(docs) {
            self.out.lines.push(if l.is_empty() { l } else { format!("{indent}{l}") });
        }
        if !docs.trim().is_empty() {
            self.out.lines.push(String::new());
        }
    }

    fn emit_member(&mut self, d: &Decl, m: &Member, indent: &str) {
        let objtype = if m.kind == MemberKind::Enumerator { "enumerator" } else { "field" };
        self.out.lines.push(format!("{indent}.. odin:{objtype}:: {}", m.signature));
        let more = format!("{indent}   ");
        // Every platform variant of a name but the first is left out of the index.
        if self.options.no_index || d.variant > 0 {
            self.out.lines.push(format!("{more}:no-index:"));
        }
        self.out.lines.push(String::new());
        self.push_docs(&m.docs, &more);
    }
}

fn auto_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Proc => "proc",
        Kind::ProcGroup => "procgroup",
        Kind::Const => "const",
        Kind::Var => "var",
        Kind::ForeignImport => "package",
        Kind::Struct | Kind::Union | Kind::Enum | Kind::BitSet | Kind::BitField | Kind::Type => {
            "type"
        }
    }
}

/// What the package's page leaves out but its public declarations name, separated by
/// spaces: private types, and private procedures a public procedure group gathers. The
/// odin domain shows them as text instead of links to descriptions that do not exist.
/// With private members documented, there are none.
fn private_names(options: &Options, pkg: &Package) -> String {
    if options.private {
        return String::new();
    }
    let mut grouped: Vec<&str> = Vec::new();
    for d in pkg.decls.iter().filter(|d| d.private == Private::Public) {
        if d.kind != Kind::ProcGroup {
            continue;
        }
        let (Some(open), Some(close)) = (d.signature.find('{'), d.signature.rfind('}')) else {
            continue;
        };
        if close < open {
            continue;
        }
        for member in d.signature[open + 1..close].split(',') {
            let member = member.trim();
            if !grouped.contains(&member) {
                grouped.push(member);
            }
        }
    }
    let mut names: Vec<&str> = Vec::new();
    for d in pkg.decls.iter().filter(|d| d.private != Private::Public) {
        let listed = match d.kind {
            Kind::Struct
            | Kind::Union
            | Kind::Enum
            | Kind::BitSet
            | Kind::BitField
            | Kind::Type => true,
            Kind::Proc | Kind::ProcGroup => grouped.contains(&d.name.as_str()),
            Kind::Const | Kind::Var | Kind::ForeignImport => false,
        };
        if listed && !names.contains(&d.name.as_str()) {
            names.push(&d.name);
        }
    }
    names.sort_unstable();
    names.join(" ")
}
