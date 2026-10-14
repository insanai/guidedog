//! What Sphinx's `ModuleAnalyzer` (`sphinx.pycode.parser.Parser`) finds in a module:
//! attribute documentation from `#:` comments and from string literals after
//! assignments, annotations, definition order, `@final`, and `@overload`.

use std::collections::{HashMap, HashSet};

use crate::literal::{cleandoc, split_lines, str_value};
use crate::scan::{clause_body, decorators_of, function_of, type_comment_text};
use crate::syntax::{Node, NodeId};
use crate::unparse::{UnparseMode, unparse};
use crate::{Function, FunctionId, Module};

/// `(scope, name)`: the scope is `""` at module level, else the class's qualname.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AttrKey {
    /// `""`, or the qualname of the class.
    pub scope: String,
    /// The attribute.
    pub name: String,
}

impl AttrKey {
    /// A key from its parts.
    pub fn new(scope: &str, name: &str) -> AttrKey {
        AttrKey { scope: scope.to_string(), name: name.to_string() }
    }
}

/// What the analyser found, keyed as Sphinx keys it.
#[derive(Debug, Clone, Default)]
pub struct Analysis {
    /// Attribute documentation: lines, ending with `""`.
    pub attr_docs: HashMap<AttrKey, Vec<String>>,
    /// The first-insertion order of `attr_docs`.
    pub attr_order: Vec<AttrKey>,
    /// Annotations as Sphinx's unparser writes them.
    pub annotations: HashMap<AttrKey, String>,
    /// The first-insertion order of `annotations`.
    pub annotation_order: Vec<AttrKey>,
    /// Dotted qualname -> order of its last definition.
    pub tagorder: HashMap<String, usize>,
    /// Qualnames decorated with `@final`.
    pub finals: HashSet<String>,
    /// Qualname -> its `@overload` variants, in order.
    pub overloads: HashMap<String, Vec<FunctionId>>,
    counter: usize,
}

impl Analysis {
    /// The documentation of `name` in `scope`, if the analyser found one.
    pub fn attr_doc(&self, scope: &str, name: &str) -> Option<&[String]> {
        self.attr_docs.get(&AttrKey::new(scope, name)).map(Vec::as_slice)
    }

    /// Whether the analyser documented `name` in `scope`.
    pub fn has_attr_doc(&self, scope: &str, name: &str) -> bool {
        self.attr_doc(scope, name).is_some()
    }
}

/// `VariableCommentPicker`'s state.
struct Picker<'a> {
    info: &'a mut Analysis,
    functions: &'a mut Vec<Function>,
    /// The enclosing definitions (Sphinx's context).
    names: Vec<String>,
    /// How many classes enclose the current point.
    classes: usize,
    /// The function being visited.
    function: Option<NodeId>,
    /// Its first parameter.
    self_name: String,
    typing_mods: Vec<String>,
    final_names: Vec<String>,
    overload_names: Vec<String>,
}

pub(crate) fn analyse(m: &mut Module) {
    let mut pk = Picker {
        info: &mut m.info,
        functions: &mut m.functions,
        names: Vec::new(),
        classes: 0,
        function: None,
        self_name: String::new(),
        typing_mods: Vec::new(),
        final_names: Vec::new(),
        overload_names: Vec::new(),
    };
    pk.pick_block(m.syntax.root());
}

impl Picker<'_> {
    fn pick_block(&mut self, block: Node<'_>) {
        let mut previous = None;
        for n in block.named_children().filter(|n| n.kind() != "comment") {
            self.pick_statement(n, previous);
            previous = Some(n);
        }
    }

    fn pick_statement(&mut self, n: Node<'_>, previous: Option<Node<'_>>) {
        match n.kind() {
            "expression_statement" => self.pick_expression(n, previous),
            "import_statement" | "import_from_statement" => self.pick_import(n),
            "class_definition" | "function_definition" => self.pick_definition(n, &[]),
            "decorated_definition" => {
                self.pick_definition(n.field("definition"), &decorators_of(n))
            }
            "if_statement" => {
                self.pick_block(n.field("consequence"));
                for alt in n.named_children() {
                    match alt.kind() {
                        "elif_clause" => self.pick_block(alt.field("consequence")),
                        "else_clause" => self.pick_block(clause_body(alt)),
                        _ => {}
                    }
                }
            }
            "try_statement" => {
                // Sphinx's parser ignores except and finally clauses.
                for part in n.named_children() {
                    match part.kind() {
                        "block" => self.pick_block(part),
                        "else_clause" => self.pick_block(clause_body(part)),
                        _ => {}
                    }
                }
            }
            "for_statement" | "while_statement" => {
                self.pick_block(n.field("body"));
                let alt = n.field("alternative");
                if !alt.is_null() {
                    self.pick_block(clause_body(alt));
                }
            }
            "with_statement" => self.pick_block(n.field("body")),
            "match_statement" => {
                for c in n.field("body").named_children().filter(|c| c.kind() == "case_clause") {
                    self.pick_block(c.field("consequence"));
                }
            }
            "type_alias_statement" => {
                let name = n.field_text("left").to_string();
                self.collect_doc_comment(n, &[name]);
            }
            _ => {}
        }
    }

    /// An assignment, or a string literal after one.
    fn pick_expression(&mut self, n: Node<'_>, previous: Option<Node<'_>>) {
        let first = n.named_child(0);
        match first.kind() {
            "assignment" => self.pick_assignment(n, first),
            "string" | "concatenated_string" => {
                let Some(previous) = previous.filter(|p| assignment_like(*p)) else { return };
                if n.named_child_count() != 1 {
                    return;
                }
                let Some(doc) = str_value(first) else { return };
                let mut names = Vec::new();
                if !self.lvar_names(first_target(previous), &mut names) {
                    return;
                }
                let doc = dedent_docstring(&doc);
                for name in &names {
                    self.add_comment(name, &doc);
                    self.add_entry(name);
                }
            }
            _ => {}
        }
    }

    fn pick_assignment(&mut self, statement: Node<'_>, first: Node<'_>) {
        let mut names = Vec::new();
        let mut annotation = None;
        let mut e = first;
        while e.kind() == "assignment" {
            if !self.lvar_names(e.field("left"), &mut names) {
                return;
            }
            annotation = annotation.or(e.field("type").id());
            e = e.field("right");
        }
        if let Some(annotation) = annotation {
            let text = unparse(statement.node_at(annotation), UnparseMode::Sphinx);
            for name in &names {
                self.add_annotation(name, &text);
            }
        } else if let Some(comment) = type_comment_after(statement) {
            // x = value  # type: T is an annotation too (ast's type_comment).
            for name in &names {
                self.add_annotation(name, comment);
            }
        }
        self.collect_doc_comment(statement, &names);
    }

    /// `get_lvar_names`: the names an assignment target defines. False when the target
    /// defines none (a subscript, or a name that is not an attribute of self inside a
    /// function), which makes Sphinx ignore the whole assignment.
    fn lvar_names(&self, target: Node<'_>, out: &mut Vec<String>) -> bool {
        let in_function = self.function.is_some();
        match target.kind() {
            "identifier" => {
                let name = target.text();
                if in_function && name != self.self_name {
                    return false;
                }
                out.push(name.to_string());
                true
            }
            "pattern_list" | "tuple_pattern" | "list_pattern" | "tuple" | "list"
            | "expression_list" => {
                for t in target.named_children() {
                    self.lvar_names(t, out);
                }
                true
            }
            "list_splat_pattern" | "list_splat" => self.lvar_names(target.named_child(0), out),
            "attribute" => {
                let object = target.field("object");
                if !in_function || object.kind() != "identifier" || object.text() != self.self_name
                {
                    return false;
                }
                out.push(target.field_text("attribute").to_string());
                true
            }
            _ => false,
        }
    }

    /// `get_qualname_for`: `None` outside the definitions Sphinx records, which are
    /// those at module and class level, and self attributes set in `__init__`.
    fn qualname_for<'s>(&'s self, name: &'s str) -> Option<Vec<&'s str>> {
        let mut out: Vec<&str> = self.names.iter().map(String::as_str).collect();
        if self.function.is_some() {
            if self.classes == 0 || out.last() != Some(&"__init__") {
                return None;
            }
            out.pop();
        }
        out.push(name);
        Some(out)
    }

    fn add_entry(&mut self, name: &str) {
        let Some(q) = self.qualname_for(name) else { return };
        let key = q.join(".");
        let order = self.info.counter;
        self.info.tagorder.insert(key, order);
        self.info.counter += 1;
    }

    fn scope_key(&self, name: &str) -> Option<AttrKey> {
        let q = self.qualname_for(name)?;
        Some(AttrKey { scope: q[..q.len() - 1].join("."), name: name.to_string() })
    }

    fn add_comment(&mut self, name: &str, comment: &str) {
        let Some(key) = self.scope_key(name) else { return };
        let mut lines: Vec<String> = Vec::new();
        if !comment.is_empty() {
            lines.extend(split_lines(comment).into_iter().map(str::to_string));
        }
        lines.push(String::new());
        if !self.info.attr_docs.contains_key(&key) {
            self.info.attr_order.push(key.clone());
        }
        self.info.attr_docs.insert(key, lines);
    }

    fn add_annotation(&mut self, name: &str, annotation: &str) {
        let Some(key) = self.scope_key(name) else { return };
        if !self.info.annotations.contains_key(&key) {
            self.info.annotation_order.push(key.clone());
        }
        self.info.annotations.insert(key, annotation.to_string());
    }

    /// Finds the `#:` comment after an assignment on its line, else the `#:` comment
    /// lines directly above it, and records the names in definition order.
    fn collect_doc_comment(&mut self, statement: Node<'_>, names: &[String]) {
        let comment = comment_after(statement).or_else(|| comments_before(statement));
        for name in names {
            if let Some(comment) = &comment {
                self.add_comment(name, comment);
            }
            self.add_entry(name);
        }
    }

    fn pick_import(&mut self, n: Node<'_>) {
        let is_from = n.kind() == "import_from_statement";
        let from = if is_from { n.field_text("module_name") } else { "" };
        let typing = from == "typing" || from == "typing_extensions";
        for child in n.children().filter(|c| c.field_name() == Some("name")) {
            let (name, alias) = if child.kind() == "aliased_import" {
                (child.field_text("name"), child.field_text("alias"))
            } else {
                (child.text(), "")
            };
            let bound = if alias.is_empty() { name } else { alias };
            self.add_entry(bound);
            let list = match name {
                "typing" | "typing_extensions" if !is_from => &mut self.typing_mods,
                "typing.final" | "typing_extensions.final" if !is_from => &mut self.final_names,
                "typing.overload" | "typing_extensions.overload" if !is_from => {
                    &mut self.overload_names
                }
                "final" if typing => &mut self.final_names,
                "overload" if typing => &mut self.overload_names,
                _ => continue,
            };
            list.push(bound.to_string());
        }
    }

    fn decorated_with(
        &self,
        n: Node<'_>,
        decorators: &[NodeId],
        word: &str,
        names: &[String],
    ) -> bool {
        decorators.iter().any(|&d| {
            let s = unparse(n.node_at(d), UnparseMode::Sphinx);
            let typing_word = self.typing_mods.iter().any(|m| s == format!("{m}.{word}"));
            typing_word || names.contains(&s)
        })
    }

    fn pick_definition(&mut self, n: Node<'_>, decorators: &[NodeId]) {
        if n.is_null() {
            return;
        }
        let name = n.field_text("name");
        if n.kind() == "class_definition" {
            self.add_entry(name);
            if self.decorated_with(n, decorators, "final", &self.final_names) {
                self.add_final(name);
            }
            self.names.push(name.to_string());
            self.classes += 1;
            self.pick_block(n.field("body"));
            self.classes -= 1;
            self.names.pop();
            return;
        }
        if self.function.is_some() {
            return; // Sphinx does not look into nested functions.
        }
        self.add_entry(name);
        if self.decorated_with(n, decorators, "final", &self.final_names) {
            self.add_final(name);
        }
        if self.decorated_with(n, decorators, "overload", &self.overload_names) {
            self.add_overload(n, name);
        }
        self.names.push(name.to_string());
        self.function = n.id();
        self.self_name = first_param_name(n).to_string();
        self.pick_block(n.field("body"));
        self.function = None;
        self.self_name.clear();
        self.names.pop();
    }

    fn add_final(&mut self, name: &str) {
        if let Some(q) = self.qualname_for(name) {
            let key = q.join(".");
            self.info.finals.insert(key);
        }
    }

    fn add_overload(&mut self, n: Node<'_>, name: &str) {
        let Some(q) = self.qualname_for(name) else { return };
        let key = q.join(".");
        let id = FunctionId(self.functions.len() as u32);
        self.functions.push(function_of(n, &key));
        self.info.overloads.entry(key).or_default().push(id);
    }
}

fn assignment_like(n: Node<'_>) -> bool {
    n.kind() == "type_alias_statement"
        || (n.kind() == "expression_statement" && n.named_child(0).kind() == "assignment")
}

fn first_target(statement: Node<'_>) -> Node<'_> {
    if statement.kind() == "type_alias_statement" {
        return statement.field("left").named_child(0);
    }
    statement.named_child(0).field("left")
}

/// The end of the line a statement ends on, from `start`.
fn line_end(s: &str, start: usize) -> usize {
    s[start..].find('\n').map_or(s.len(), |k| start + k)
}

/// The `#:` comment that ends the statement's last line.
fn comment_after(statement: Node<'_>) -> Option<String> {
    let s = statement.source();
    let b = s.as_bytes();
    let mut i = statement.end_byte();
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\x0c' | b';') {
        i += 1;
    }
    if i >= b.len() || b[i] != b'#' {
        return None;
    }
    doc_comment(&s[i..line_end(s, i)]).map(str::to_string)
}

/// The `# type: T` comment that ends an assignment's line.
fn type_comment_after(statement: Node<'_>) -> Option<&str> {
    let s = statement.source();
    let b = s.as_bytes();
    let mut i = statement.end_byte();
    while i < b.len() && matches!(b[i], b' ' | b'\t') {
        i += 1;
    }
    if i >= b.len() || b[i] != b'#' {
        return None;
    }
    type_comment_text(&s[i..line_end(s, i)])
}

/// Sphinx's `comment_re`, `^\s*#: ?(.*)`: the text after `#:` and one optional space.
fn doc_comment(line: &str) -> Option<&str> {
    let t = line.trim_start().strip_prefix("#:")?;
    let t = t.strip_prefix(' ').unwrap_or(t);
    Some(t.trim_end_matches('\r'))
}

/// The `#:` lines directly above a statement that starts its line.
fn comments_before(statement: Node<'_>) -> Option<String> {
    let s = statement.source();
    let start = statement.start_byte();
    let line_start = s[..start].rfind('\n').map_or(0, |k| k + 1);
    if !s[line_start..start].trim().is_empty() {
        return None;
    }
    let mut found = Vec::new();
    let mut end = line_start.saturating_sub(1);
    while end > 0 {
        let begin = s[..end].rfind('\n').map_or(0, |k| k + 1);
        let Some(comment) = doc_comment(&s[begin..end]) else { break };
        found.push(comment);
        end = begin.saturating_sub(1);
    }
    if found.is_empty() {
        return None;
    }
    found.reverse();
    Some(dedent_docstring(&found.join("\n")))
}

/// `sphinx.pycode.parser.dedent_docstring`: `inspect.cleandoc` without leading and
/// trailing line breaks.
fn dedent_docstring(s: &str) -> String {
    cleandoc(s).trim_matches(|c| c == '\r' || c == '\n').to_string()
}

/// The name of a function's first positional parameter, `""` if none.
fn first_param_name(n: Node<'_>) -> &str {
    let Some(p) = n.field("parameters").named_children().next() else { return "" };
    match p.kind() {
        "identifier" => p.text(),
        "default_parameter" | "typed_default_parameter" => p.field_text("name"),
        "typed_parameter" if p.named_child(0).kind() == "identifier" => p.named_child(0).text(),
        _ => "",
    }
}
