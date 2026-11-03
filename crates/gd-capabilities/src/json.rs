//! The JSON form of the report.

use std::fmt::Write;

use gd_report::escape_json;

use crate::{
    Adapter, BuilderCapability, Capabilities, DirectiveCapability, ExtensionCapability,
    FormatCapability, PluginCapability, RoleCapability,
};

/// Appends the JSON form of `report` to `out`: one object whose keys are the fields of
/// [`Capabilities`] (`schema`, `guidedog`, `source_map_version`, `readers`,
/// `renderers`, `formats`, `builders`, `directives`, `roles`, `markdown_extensions`,
/// `sphinx_extensions`, `plugins`), each list an array of objects keyed by the fields of
/// its entry type; `enabled_by` is `default`, `config` or `plugin`.
pub fn write_json(report: &Capabilities, out: &mut String) {
    let _ = write!(
        out,
        "{{\"schema\":{},\"guidedog\":\"{}\",\"source_map_version\":{}",
        report.schema, report.guidedog, report.source_map_version
    );
    list(out, "readers", &report.readers, adapter);
    list(out, "renderers", &report.renderers, adapter);
    list(out, "formats", &report.formats, format);
    list(out, "builders", &report.builders, builder);
    list(out, "directives", &report.directives, directive);
    list(out, "roles", &report.roles, role);
    list(out, "markdown_extensions", &report.markdown_extensions, extension);
    list(out, "sphinx_extensions", &report.sphinx_extensions, extension);
    list(out, "plugins", &report.plugins, plugin);
    out.push_str("}\n");
}

/// Writes `,"key":[...]` with `write` for each item.
fn list<T>(out: &mut String, key: &str, items: &[T], write: fn(&mut Object<'_>, &T)) {
    let _ = write!(out, ",\"{key}\":[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let mut object = Object { out, first: true };
        object.out.push('{');
        write(&mut object, item);
        object.out.push('}');
    }
    out.push(']');
}

/// An object being written: its members, separated by commas.
struct Object<'a> {
    out: &'a mut String,
    first: bool,
}

impl Object<'_> {
    fn key(&mut self, key: &str) {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        let _ = write!(self.out, "\"{key}\":");
    }

    fn text(&mut self, key: &str, value: &str) {
        self.key(key);
        escape_json(self.out, value);
    }

    fn flag(&mut self, key: &str, value: bool) {
        self.key(key);
        let _ = write!(self.out, "{value}");
    }

    fn texts<S: AsRef<str>>(&mut self, key: &str, values: &[S]) {
        self.key(key);
        self.out.push('[');
        for (i, v) in values.iter().enumerate() {
            if i > 0 {
                self.out.push(',');
            }
            escape_json(self.out, v.as_ref());
        }
        self.out.push(']');
    }
}

fn adapter(o: &mut Object<'_>, a: &Adapter) {
    o.text("id", a.id);
    o.text("name", a.name);
    o.text("version", a.version);
    o.text("conformance", a.conformance);
}

fn format(o: &mut Object<'_>, f: &FormatCapability) {
    o.text("name", f.name);
    o.text("reader", f.reader);
    o.texts("suffixes", &f.suffixes);
    o.flag("source_map", f.source_map);
    o.texts("block_kinds", &f.block_kinds);
    o.texts("operations", &f.operations);
}

fn builder(o: &mut Object<'_>, b: &BuilderCapability) {
    o.text("name", b.name);
    o.text("format", b.format);
    o.flag("preview", b.preview);
    o.flag("serve", b.serve);
    o.flag("output_source_map", b.output_source_map);
    o.flag("dependency_manifest", b.dependency_manifest);
}

fn directive(o: &mut Object<'_>, d: &DirectiveCapability) {
    o.text("name", &d.name);
    o.text("origin", &d.origin);
    o.text("enabled_by", d.enabled_by.name());
    o.text("extension", d.extension);
    o.flag("enabled", d.enabled);
    o.texts("formats", &d.formats);
    o.text("argument", d.argument);
    o.text("content", d.content);
    o.key("options");
    o.out.push('[');
    for (i, option) in d.options.iter().enumerate() {
        if i > 0 {
            o.out.push(',');
        }
        let mut inner = Object { out: &mut *o.out, first: true };
        inner.out.push('{');
        inner.text("name", option.name);
        inner.text("kind", option.kind);
        inner.texts("choices", &option.choices);
        inner.out.push('}');
    }
    o.out.push(']');
}

fn role(o: &mut Object<'_>, r: &RoleCapability) {
    o.text("name", &r.name);
    o.text("origin", &r.origin);
    o.text("enabled_by", r.enabled_by.name());
    o.text("extension", r.extension);
    o.flag("enabled", r.enabled);
}

fn extension(o: &mut Object<'_>, e: &ExtensionCapability) {
    o.text("name", e.name);
    o.flag("default", e.default);
    o.flag("enabled", e.enabled);
}

fn plugin(o: &mut Object<'_>, p: &PluginCapability) {
    o.text("id", &p.id);
    o.text("kind", p.kind);
    o.text("version", &p.version);
    o.texts("commands", &p.commands);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_is_valid_json_with_every_section() {
        let report = crate::load_capabilities(&gd_plugins::Registry::default(), None);
        let mut out = String::new();
        write_json(&report, &mut out);
        let value = gd_json::parse(&out).unwrap();
        for key in ["readers", "formats", "builders", "directives", "roles", "plugins"] {
            assert!(value.get(key).and_then(|v| v.as_array()).is_some(), "{key}");
        }
        let formats = value.get("formats").and_then(|v| v.as_array()).unwrap();
        let ops = formats[0].get("operations").and_then(|v| v.as_array()).unwrap();
        assert_eq!(ops[0].as_str(), Some("replace-text"));
        let directives = value.get("directives").and_then(|v| v.as_array()).unwrap();
        let image =
            directives.iter().find(|d| d.get("name").and_then(|n| n.as_str()) == Some("image"));
        let options = image.and_then(|d| d.get("options")).and_then(|o| o.as_array()).unwrap();
        assert!(options.iter().any(|o| o.get("name").and_then(|n| n.as_str()) == Some("align")));
    }
}
