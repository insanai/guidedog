//! Domain objects: a Desc node back to `.. domain:objtype:: signature`, and the info
//! fields Sphinx rebuilt back to `:param type name: text`.

use guidedoc::{NodeFlag, NodeId, NodeKind};

use crate::blocks::Opt;
use crate::state::Writer;

/// The field name each docfield type is written with.
fn field_name(docfield: &str, domain: &str) -> &'static str {
    match docfield {
        "parameter" | "arguments" => "param",
        "variable" => "var",
        "exceptions" if domain == "cpp" => "throws",
        "exceptions" => "raises",
        "errors" => "throws",
        "returnvalue" => "returns",
        "returntype" => "rtype",
        "result" => "result",
        "retval" => "retval",
        "template parameter" => "tparam",
        _ => "field",
    }
}

impl Writer<'_, '_> {
    /// `.. domain:objtype:: signature` with one line per signature, its options, and
    /// its content.
    pub fn desc(&mut self, id: NodeId) {
        let domain = self.custom(id, "domain").unwrap_or("").to_owned();
        let objtype = self.custom(id, "objtype").unwrap_or("").to_owned();
        let mut signatures = Vec::new();
        let mut module = None;
        let mut content = None;
        for child in self.doc.children(id) {
            match self.kind(child) {
                NodeKind::DescSignature => {
                    module = module.or_else(|| self.custom(child, "module"));
                    signatures.push(self.signature_text(child, &domain));
                }
                NodeKind::DescContent => content = Some(child),
                _ => {}
            }
        }
        let mut options: Vec<Opt> = Vec::new();
        self.module_context(&domain, module, &mut options);
        for (attr, option) in [("noindex", "no-index"), ("no-contents-entry", "no-contents-entry")]
        {
            if self.custom(id, attr).is_some() {
                options.push((option, String::new()));
            }
        }
        self.common_options(id, &[], &mut options);
        let name = if domain.is_empty() { objtype } else { format!("{domain}:{objtype}") };
        let first = signatures.first().cloned().unwrap_or_default();
        self.directive_head(&name, &first, &options);
        if signatures.len() > 1 {
            self.lossy(id, "an object with several signatures", "each signature on its line");
        }
        if let Some(content) = content {
            self.directive_body(content, 0);
        }
    }

    /// Writes the module an object belongs to: `:module:` for Python, the package
    /// directive for Odin, when it differs from the one in effect.
    fn module_context(&mut self, domain: &str, module: Option<&str>, options: &mut Vec<Opt>) {
        let Some(module) = module else { return };
        let current = self.modules.iter().find(|(d, _)| d == domain).map(|(_, m)| m.as_str());
        if current == Some(module) {
            return;
        }
        match domain {
            "py" => options.push(("module", module.to_owned())),
            "odin" => {
                self.directive_head("odin:package", module, &[("no-index", String::new())]);
                self.modules.retain(|(d, _)| d != domain);
                self.modules.push((domain.to_owned(), module.to_owned()));
            }
            "rust" => {
                self.directive_head("rust:module", module, &[("no-index", String::new())]);
                self.modules.retain(|(d, _)| d != domain);
                self.modules.push((domain.to_owned(), module.to_owned()));
            }
            _ => {}
        }
    }

    /// A signature as its directive takes it, rebuilt from its parts: the parameter
    /// lists in parentheses, the return annotation after "->". Python's module prefix
    /// and annotations come from options, so they are left out.
    fn signature_text(&self, id: NodeId, domain: &str) -> String {
        let module = self.custom(id, "module").unwrap_or("");
        let mut out = String::new();
        for part in self.doc.children(id) {
            match self.kind(part) {
                NodeKind::DescAddname => {
                    let text = self.plain(part);
                    if text.strip_suffix('.') != Some(module)
                        && text.strip_suffix("::") != Some(module)
                    {
                        out.push_str(&text);
                    }
                }
                NodeKind::DescAnnotation if domain == "py" => {}
                NodeKind::DescParameterlist => {
                    out.push('(');
                    self.parameters(part, &mut out);
                    out.push(')');
                }
                NodeKind::DescReturns => {
                    out.push_str(" -> ");
                    out.push_str(&self.plain(part));
                }
                NodeKind::Index => {}
                _ => out.push_str(&self.plain(part)),
            }
        }
        out.trim().replace('\n', " ")
    }

    fn parameters(&self, list: NodeId, out: &mut String) {
        for (n, p) in self.doc.children(list).enumerate() {
            if self.kind(p) == NodeKind::DescOptional {
                out.push('[');
                if n > 0 {
                    out.push_str(", ");
                }
                self.parameters(p, out);
                out.push(']');
                continue;
            }
            if n > 0 {
                out.push_str(", ");
            }
            out.push_str(&self.plain(p));
        }
    }

    /// The plain text of a subtree.
    pub fn plain(&self, id: NodeId) -> String {
        let mut text = String::new();
        self.doc.plain_text(id, &mut text);
        text
    }

    /// An info field Sphinx rebuilt: one `:name type arg: text` field per use.
    pub fn doc_field(&mut self, field: NodeId) {
        let docfield = self.custom(field, "docfield").unwrap_or("");
        let domain = self.enclosing_domain(field);
        let name = field_name(docfield, &domain);
        let Some(body) = self.doc.children(field).find(|&c| self.kind(c) == NodeKind::FieldBody)
        else {
            return;
        };
        let uses: Vec<NodeId> = match self.grouped_uses(body) {
            Some(items) => items,
            None => vec![body],
        };
        for (n, item) in uses.into_iter().enumerate() {
            if n > 0 {
                self.blank = false;
            }
            self.field_use(name, item);
        }
    }

    /// The domain of the object whose content holds `id`.
    fn enclosing_domain(&self, id: NodeId) -> String {
        let mut at = self.doc.parent_of(id);
        while let Some(p) = at {
            if self.kind(p) == NodeKind::Desc {
                return self.custom(p, "domain").unwrap_or("").to_owned();
            }
            at = self.doc.parent_of(p);
        }
        String::new()
    }

    /// The list items of a field body that holds one use per item.
    fn grouped_uses(&self, body: NodeId) -> Option<Vec<NodeId>> {
        let mut children = self.doc.children(body);
        let list = children.next().filter(|_| children.next().is_none())?;
        if self.kind(list) != NodeKind::List {
            return None;
        }
        let items: Vec<NodeId> = self.doc.children(list).collect();
        let generated = |item: NodeId| {
            let para = self.doc.first_child(item).filter(|&p| self.kind(p) == NodeKind::Paragraph);
            para.and_then(|p| self.doc.first_child(p))
                .is_some_and(|c| self.doc.node(c).flags.has(NodeFlag::Generated))
        };
        items.iter().all(|&i| generated(i)).then_some(items)
    }

    /// One use: its generated head ("a (int) – ") becomes the field's argument and
    /// type; the rest of the paragraph and the blocks after it, the body.
    fn field_use(&mut self, name: &str, item: NodeId) {
        let para = self.doc.first_child(item).filter(|&p| self.kind(p) == NodeKind::Paragraph);
        let mut head = String::new();
        let mut content_starts = 0;
        if let Some(p) = para {
            for (n, c) in self.doc.children(p).enumerate() {
                if !self.doc.node(c).flags.has(NodeFlag::Generated) {
                    break;
                }
                self.doc.plain_text(c, &mut head);
                content_starts = n + 1;
            }
        }
        let has_content = para.is_some_and(|p| self.doc.child_count(p) > content_starts);
        let (arg, ty) = split_head(&head);
        let mut marker = format!(":{name}");
        let argument_field = !matches!(name, "returns" | "rtype");
        if argument_field {
            for part in [ty, arg] {
                if !part.is_empty() {
                    marker.push(' ');
                    marker.push_str(part);
                }
            }
        }
        marker.push_str(": ");
        self.block_start();
        self.push_frame(3, &marker);
        if let Some(p) = para {
            let lines = if has_content {
                self.collect_skipping(p, content_starts)
            } else if argument_field {
                Vec::new()
            } else {
                vec![head.trim().to_owned()]
            };
            for line in &lines {
                self.line(line);
            }
            self.blank = true;
        }
        let rest = usize::from(para.is_some());
        for block in self.doc.children(item).skip(rest) {
            self.node(block);
        }
        self.pop_frame();
        self.block_end();
    }

    /// Collects a paragraph's inline children after the first `skip`.
    fn collect_skipping(&mut self, p: NodeId, skip: usize) -> Vec<String> {
        let saved = std::mem::take(&mut self.inline);
        let saved_mode = self.in_inline;
        self.inline.clear(false);
        self.in_inline = true;
        for child in self.doc.children(p).skip(skip) {
            self.node(child);
        }
        self.in_inline = saved_mode;
        let first = self.settings.width.saturating_sub(self.first_indentation());
        let lines = self.inline.wrap(
            first.max(self.settings.width / 3),
            self.available(),
            self.tight_wrap,
        );
        self.inline = saved;
        lines
    }
}

/// Splits a use's head "arg (type) – " into the argument and the type.
fn split_head(head: &str) -> (&str, &str) {
    let head = head.trim_end();
    let head =
        head.strip_suffix('–').or_else(|| head.strip_suffix("--")).unwrap_or(head).trim_end();
    if let Some(open) = head.find(" (")
        && head.ends_with(')')
    {
        return (head[..open].trim(), &head[open + 2..head.len() - 1]);
    }
    (head.trim(), "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads_split_into_argument_and_type() {
        assert_eq!(split_head("a (int) – "), ("a", "int"));
        assert_eq!(split_head("ValueError – "), ("ValueError", ""));
        assert_eq!(split_head(""), ("", ""));
    }
}
