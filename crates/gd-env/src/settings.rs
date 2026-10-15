//! The settings digests: what, besides a document's own source, changes how it reads
//! (`reading_settings`, `build_settings`), and what changes every output of a builder
//! (`output_settings`). docs/manual/internals/incremental.rst lists every input.

use std::path::Path;
use std::sync::OnceLock;

use gd_conf::{Config, Digest, Digester};
use guidedoc::Limits;

use crate::builder::BuilderKind;

/// The digest of everything in the configuration that changes how a document reads;
/// when it changes, every document is read again. The engine's version and schema and
/// [`executable_stamp`] are in it, so a new Guidedog never reuses what an older one read.
pub fn reading_settings(c: &Config) -> Digest {
    let mut d = Digester::new();
    d.text(guidedoc::VERSION).number(i64::from(guidedoc::SCHEMA_VERSION)).text(executable_stamp());
    for text in [
        &c.language,
        &c.rst_prolog,
        &c.rst_epilog,
        &c.default_role,
        &c.primary_domain,
        &c.highlight_language,
        &c.today,
        &c.today_fmt,
        &c.version,
        &c.release,
        &c.project,
        &c.toc_object_entries_show_parents,
        &c.manpages_url,
        &c.intersphinx.resolve_self,
    ] {
        d.text(text);
    }
    for flag in [
        c.smartquotes,
        c.trim_footnote_reference_space,
        c.todo_include_todos,
        c.show_authors,
        c.numfig,
        c.math_number_all,
        c.add_function_parentheses,
        c.add_module_names,
        c.toc_object_entries,
        c.option_emphasise_placeholders,
        c.trim_doctest_flags,
        c.has_extension("sphinx.ext.intersphinx"),
    ] {
        d.flag(flag);
    }
    d.texts(&c.source_suffix).texts(&c.include_roots);
    for t in &c.object_types {
        d.text(&t.directive).text(&t.role).text(&t.index).text(&t.display);
        d.flag(t.first_word).flag(t.program).flag(t.target);
    }
    for (alias, directive) in &c.directive_aliases {
        d.text(alias).text(directive);
    }
    for (name, (url, caption)) in &c.extlinks {
        d.text(name).text(url).text(caption);
    }
    myst_settings(&mut d, c);
    api_settings(&mut d, c);
    d.finish()
}

fn myst_settings(d: &mut Digester, c: &Config) {
    let m = &c.myst;
    d.texts(&m.enable_extensions).number(m.heading_anchors).number(m.words_per_minute);
    for flag in [
        m.dmath_allow_labels,
        m.dmath_allow_space,
        m.dmath_allow_digits,
        m.dmath_double_inline,
        m.linkify_fuzzy_links,
        m.footnote_transition,
        m.commonmark_only,
        m.dmath_enable,
        m.amsmath_enable,
    ] {
        d.flag(flag);
    }
    for (k, v) in m.substitutions.iter().chain(&m.html_meta) {
        d.text(k).text(v);
    }
    d.texts(&m.sub_delimiters).texts(&m.url_schemes).texts(&m.disable_syntax);
}

/// The settings of the API generators: autodoc, the odin pages, the rust pages.
fn api_settings(d: &mut Digester, c: &Config) {
    let a = &c.autodoc;
    d.flag(c.has_extension("sphinx.ext.autodoc")).texts(&a.source_paths);
    for text in [
        &a.autoclass_content,
        &a.class_signature,
        &a.member_order,
        &a.typehints,
        &a.typehints_description_target,
        &a.typehints_format,
    ] {
        d.text(text);
    }
    for o in &a.default_options {
        d.text(&o.name).text(&o.value).flag(o.flag);
    }
    for flag in [
        a.docstring_signature,
        a.inherit_docstrings,
        a.preserve_defaults,
        a.python_display_short_literal_types,
        a.strip_signature_backslash,
        a.use_type_comments,
    ] {
        d.flag(flag);
    }
    let o = &c.odin_api;
    d.texts(&o.dirs).text(&o.root).texts(&o.options).text(&o.member_order);
    d.flag(o.generate_api_docs).flag(o.add_toctree_entry);
    for (name, dir) in &o.collections {
        d.text(name).text(dir);
    }
    let r = &c.rust_api;
    d.texts(&r.crates).text(&r.root).texts(&r.options).text(&r.member_order);
    d.flag(r.generate_api_docs).flag(r.add_toctree_entry);
}

/// The digest the environment is saved with: [`reading_settings`] and what the build
/// adds to it, the translations (`intl`, gd-intl's digest of the language, catalogs and
/// labels), trust, the engine limits, and the build plugins (`PluginRun::digest`).
pub fn build_settings(
    reading: Digest,
    intl: Digest,
    untrusted: bool,
    limits: &Limits,
    plugins: Digest,
) -> Digest {
    let mut d = Digester::new();
    d.digest(&reading).digest(&intl).flag(untrusted).text(&format!("{limits:?}"));
    d.digest(&plugins).finish()
}

/// What changes every output of a builder: the whole of conf.toml and `-D`
/// (`gd_conf::Loaded::settings`), the builder, the `-t` tags `only` tests (sorted),
/// whether every reference must resolve (`-n`), trust, the limits, the executable, and
/// the build plugins.
pub fn output_settings(
    project: Digest,
    kind: BuilderKind,
    tags: &[String],
    nitpicky: bool,
    untrusted: bool,
    limits: &Limits,
    plugins: Digest,
) -> Digest {
    let mut sorted = tags.to_vec();
    sorted.sort();
    let mut d = Digester::new();
    d.digest(&project).text(kind.name()).texts(&sorted).flag(nitpicky).flag(untrusted);
    d.text(&format!("{limits:?}")).text(executable_stamp()).digest(&plugins).finish()
}

/// Identifies this build of Guidedog (its executable's size and modification time),
/// so a new executable never reuses what an older one produced; `"unknown"` when the
/// executable cannot be found. Computed once per process.
pub fn executable_stamp() -> &'static str {
    static STAMP: OnceLock<String> = OnceLock::new();
    STAMP.get_or_init(|| {
        let Ok(path) = std::env::current_exe() else { return "unknown".into() };
        let Ok(info) = std::fs::metadata(&path) else { return path.display().to_string() };
        let nanos = info
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        format!("{} {nanos}", info.len())
    })
}

/// The digest of a file's bytes, read in chunks, so nothing of the file is held,
/// whatever its size: the same as `Digest::of_parts(&[bytes])`. `None` when the file
/// cannot be read.
pub fn file_digest(path: &Path) -> Option<Digest> {
    let mut hash = gd_sha::Sha256::new();
    let read = gd_host::each_chunk(path, |chunk| {
        hash.update(chunk);
        true
    });
    if !read {
        return None;
    }
    hash.update(&[0]);
    Some(Digest(hash.finish()))
}
