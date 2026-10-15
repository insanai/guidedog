//! The odin domain: signatures, ids, index and table of contents entries, roles, info
//! fields, nesting, and the generating directives (ported from the Odin reader's
//! odin_test.odin).

use std::sync::{Arc, Mutex};

use guidedoc::{
    Workspace, default_request, dump_to_string, read_document, read_extension, trusted_policy,
};
use guidedoc_sphinx::{
    Autodoc, AutodocProvider, AutodocRequest, AutodocResult, Config, TocParents, reader,
};

/// Reads `text` with `config`: the dump (empty when the read failed) and the report
/// codes, space-separated.
fn read_with(text: &str, config: &Config) -> (String, String) {
    let mut request = default_request("test.rst", text, "sphinx", "html");
    request.read.promote_title = false;
    request.policy = trusted_policy();
    request.read.extension = Some(read_extension(config));
    let mut ws = Workspace::new();
    let dump = match read_document(&mut ws, &reader(), &request) {
        Ok(doc) => dump_to_string(doc),
        Err(_) => String::new(),
    };
    let codes: Vec<&str> = ws.reports().items().iter().map(|d| d.code).collect();
    (dump, codes.join(" "))
}

fn read_text(text: &str) -> String {
    read_with(text, &Config::default()).0
}

/// Checks that the dump holds every fragment, in order.
#[track_caller]
fn expect_contains_with(text: &str, fragments: &[&str], config: &Config) {
    let (got, codes) = read_with(text, config);
    assert!(!got.is_empty(), "read failed: {codes}");
    let mut at = 0;
    for f in fragments {
        match got[at..].find(f) {
            Some(k) => at += k + f.len(),
            None => panic!("missing {f:?} (in order); got:\n{got}"),
        }
    }
}

#[track_caller]
fn expect_contains(text: &str, fragments: &[&str]) {
    expect_contains_with(text, fragments, &Config::default());
}

#[test]
fn procedure_signature() {
    let text = ".. odin:package:: core:strings\n   :imports: rt=base:runtime\n\n\
        .. odin:procedure:: @(require_results) clone :: proc(s: string, \
        loc := #caller_location) -> (res: string, err: rt.Allocator_Error) \
        #optional_allocator_error\n";
    let want = "document value=1\n  \
        index @id=\"odin-package-core-strings\" @entries=\"single: core:strings \
        (package)\" @module=\"core:strings\" @domain=\"odin\"\n  \
        desc @domain=\"odin\" @objtype=\"procedure\"\n    \
        desc-signature @id=\"odin-core-strings.clone\" @fullname=\"clone\" \
        @module=\"core:strings\" @toc=\"clone\"\n      \
        desc-annotation\n        text \"@(require_results) \"\n      \
        desc-addname\n        text \"strings.\"\n      \
        desc-name\n        text \"clone\"\n      \
        desc-annotation\n        text \" :: proc\"\n      \
        desc-parameterlist\n        \
        desc-parameter\n          text \"s\"\n          text \": \"\n          \
        text \"string\"\n        \
        desc-parameter\n          text \"loc\"\n          text \" := \"\n          \
        text \"#caller_location\"\n      \
        text \" -> \"\n      \
        desc-parameterlist\n        \
        desc-parameter\n          text \"res\"\n          text \": \"\n          \
        text \"string\"\n        \
        desc-parameter\n          text \"err\"\n          text \": \"\n          \
        xref link=unresolved name=\"base:runtime.Allocator_Error\" \
        @reftype=\"odin:type\" @odin:package=\"core:strings\"\n            \
        text \"rt.Allocator_Error\"\n      \
        text \" \"\n      text \"#optional_allocator_error\"\n    \
        index @entries=\"single: clone (procedure in core:strings)\" \
        @targets=\"odin-core-strings.clone\"\n    \
        desc-content\n";
    let (got, codes) = read_with(text, &Config::default());
    assert_eq!(got, want, "dump differs ({codes}); got:\n{got}");
}

#[test]
fn short_forms_and_polymorphic_names() {
    // "name(params)" is a procedure's short form; $T and T are never links, and a
    // procedure type's grouped names ("a, b: int") are names.
    expect_contains(
        ".. odin:proc:: map_keys($K, $V: typeid, m: map[K]V, cb: proc(a, b: K) -> bool) \
         -> []K where K == int\n",
        &[
            "desc @domain=\"odin\" @objtype=\"proc\"",
            "@id=\"odin-map_keys\"",
            "text \" :: proc\"",
            "text \"$K\"",
            "text \"$V\"",
            "text \"typeid\"",
            "text \"map[K]V\"",
            "text \"a\"",
            "text \", \"",
            "text \"b\"",
            "text \" -> \"",
            "text \"[]K\"",
            "text \"where K == int\"",
            "single: map_keys (procedure)",
        ],
    );
    let got = read_text(".. odin:proc:: f($T: typeid, x: T) -> T\n");
    assert!(!got.contains("xref"), "polymorphic names linked:\n{got}");
}

#[test]
fn values_in_types_stay_text() {
    // An array's length and an inline enumeration's names are values; a bit set's
    // brackets and a map's key hold types.
    let got = read_text(
        ".. odin:var:: q: [QUEUE_SIZE]Token\n\
         .. odin:var:: mode: enum u8 {Per_Directory, Single}\n\
         .. odin:var:: m: map[Key]bit_set[Flag; u8]\n",
    );
    for value in ["name=\"QUEUE_SIZE\"", "name=\"Per_Directory\"", "name=\"Single\""] {
        assert!(!got.contains(value), "{value} linked:\n{got}");
    }
    for name in ["name=\"Token\"", "name=\"Key\"", "name=\"Flag\""] {
        assert!(got.contains(name), "{name} not linked:\n{got}");
    }
}

#[test]
fn types_nest_their_members() {
    expect_contains(
        ".. odin:package:: readers/sphinx\n   :imports: gd=core\n\n\
         .. odin:struct:: Small :: struct($N: int, $T: typeid) #packed\n\n\
         \x20  .. odin:field:: data: [N]T `json:\"d\"`\n\n\
         \x20  .. odin:field:: node: gd.Node_Id\n\n\
         \x20     See :odin:field:`data`.\n\n\
         .. odin:enum:: Kind :: enum u8\n\n   .. odin:enumerator:: Rst = 1\n",
        &[
            "@id=\"odin-readers-sphinx.Small\"",
            "text \" :: struct\"",
            "desc-parameterlist",
            "text \"$N\"",
            "text \"#packed\"",
            "@id=\"odin-readers-sphinx.Small.data\" @fullname=\"Small.data\"",
            "text \"[N]T\"",
            "text \"`json:\\\"d\\\"`\"",
            "single: Small.data (field in readers/sphinx)",
            "xref link=unresolved name=\"core.Node_Id\" @reftype=\"odin:type\" \
             @odin:package=\"readers/sphinx\" @odin:type=\"Small\"",
            "xref link=unresolved name=\"data\" @reftype=\"odin:field\" \
             @odin:package=\"readers/sphinx\" @odin:type=\"Small\"",
            "@id=\"odin-readers-sphinx.Kind\"",
            "text \" :: enum\"",
            "text \"u8\"",
            "@id=\"odin-readers-sphinx.Kind.Rst\" @fullname=\"Kind.Rst\"",
            "text \" = \"",
            "single: Kind.Rst (enumerator in readers/sphinx)",
        ],
    );
}

#[test]
fn other_objects() {
    expect_contains(
        ".. odin:currentpackage:: fmt\n\n\
         .. odin:union:: Value :: union #no_nil {i64, Builder}\n   :availability: \
         Windows, Linux\n\n\
         .. odin:procgroup:: to_lower :: proc{to_lower_a, fmt.to_lower_b}\n\
         .. odin:bitset:: Flags :: bit_set[Flag; u8]\n\
         .. odin:bitfield:: Bits :: bit_field u32\n\n   .. odin:field:: low: u8 | 3\n\n\
         .. odin:type:: Handle :: distinct uintptr\n\
         .. odin:const:: MAX :: 64\n\
         .. odin:var:: table: map[string]Kind = nil\n\
         .. odin:foreign:: libc \"system:c\"\n\
         .. odin:procedure:: puts(s: cstring) -> i32\n   :foreign: libc\n   :private:\n\
         \x20  :deprecated: use print\n",
        &[
            "text \" :: union\"",
            "text \"#no_nil {i64, \"",
            "name=\"Builder\"",
            "strong +generated\n          text +generated \"Availability\"",
            "text \"Windows, Linux\"",
            "text \".\"",
            "name=\"to_lower_a\" @reftype=\"odin:proc\"",
            "name=\"fmt.to_lower_b\"",
            "text \" :: bit_set\"",
            "name=\"Flag\"",
            "text \"; u8]\"",
            "text \" :: bit_field\"",
            "@fullname=\"Bits.low\"",
            "text \"u8\"",
            "text \"| 3\"",
            "text \" :: distinct\"",
            "text \"uintptr\"",
            "single: Handle (type in fmt)",
            "text \" :: \"",
            "text \"64\"",
            "single: MAX (constant in fmt)",
            "text \"map[string]\"",
            "name=\"Kind\"",
            "text \"= nil\"",
            "text \"foreign import \"",
            "desc-name\n        text \"libc\"",
            "text \"\\\"system:c\\\"\"",
            "single: libc (foreign import in fmt)",
            "text \"@(private) @(deprecated=\\\"use print\\\") foreign libc \"",
            "text \"i32\"",
        ],
    );
}

#[test]
fn roles_and_toc_entries() {
    expect_contains(
        ".. odin:package:: core:fmt\n   :imports: rt=base:runtime\n\n\
         .. odin:struct:: Info\n\n   .. odin:field:: n: int\n\n\
         :odin:pkg:`core:fmt` :odin:proc:`~core:fmt.println` :odin:type:`.Info` \
         :odin:type:`rt.Allocator` :odin:obj:`Info.n`\n",
        &[
            "@toc=\"Info\"",
            "@toc=\"Info.n\"",
            "xref link=unresolved name=\"core:fmt\" @reftype=\"odin:pkg\"",
            "code \"core:fmt\"",
            "xref link=unresolved name=\"core:fmt.println\" @reftype=\"odin:proc\"",
            "code \"println\"",
            "xref link=unresolved name=\"Info\" @reftype=\"odin:type\" @refspecific=\"1\"",
            "xref link=unresolved name=\"base:runtime.Allocator\"",
            "code \"rt.Allocator\"",
            "name=\"Info.n\" @reftype=\"odin:obj\"",
        ],
    );
    let text =
        ".. odin:package:: core:fmt\n\n.. odin:struct:: Info\n\n   .. odin:field:: n: int\n";
    let all = Config { toc_object_entries_show_parents: TocParents::All, ..Config::default() };
    expect_contains_with(text, &["@toc=\"core:fmt.Info\"", "@toc=\"core:fmt.Info.n\""], &all);
    let hide = Config {
        toc_object_entries_show_parents: TocParents::Hide,
        add_module_names: false,
        ..Config::default()
    };
    let (got, _) = read_with(text, &hide);
    assert!(got.contains("@toc=\"n\"") && !got.contains("desc-addname"), "hide:\n{got}");
}

#[test]
fn fields_and_no_index() {
    expect_contains(
        ".. odin:proc:: f(s: string) -> (n: int, ok: bool)\n\n   :param s: Text.\n\
         \x20  :type s: string\n   :result n: Count.\n   :result ok: Whether.\n\
         \x20  :returns: More.\n   :rtype: Kind\n",
        &[
            "field @docfield=\"parameter\"",
            "text +generated \"Parameters\"",
            "reftype=\"odin:type\"",
            "field @docfield=\"result\"",
            "text +generated \"Returns\"",
            "text +generated \"n\"",
            "text +generated \"ok\"",
            "field @docfield=\"returnvalue\"",
            "field @docfield=\"returntype\"",
            "name=\"Kind\" @reftype=\"odin:type\"",
        ],
    );
    let got =
        read_text(".. odin:proc:: f()\n   :no-index:\n\n.. odin:package:: p\n   :no-index:\n");
    assert!(!got.contains("@id=") && !got.contains("index "), "no-index:\n{got}");
    // A second signature is the same object on other targets: no id, no entry.
    let got = read_text(".. odin:proc:: f(a: int)\n              f(a: i64)\n");
    assert_eq!(got.matches("@id=").count(), 1, "variants:\n{got}");
    assert_eq!(got.matches("single:").count(), 1, "variants:\n{got}");
}

/// Records the request it answers and answers with one procedure.
#[derive(Default)]
struct Recorder {
    seen: Mutex<(String, String, String)>,
}

impl AutodocProvider for Recorder {
    fn generate(&self, request: &AutodocRequest<'_>) -> AutodocResult {
        if let Ok(mut seen) = self.seen.lock() {
            *seen = (
                request.directive.to_owned(),
                request.argument.to_owned(),
                request.module.to_owned(),
            );
        }
        AutodocResult {
            name: "<odin>".to_owned(),
            text: ".. odin:proc:: made()\n".to_owned(),
            messages: Vec::new(),
        }
    }
}

#[test]
fn generating_directives_need_a_host() {
    let (_, codes) = read_with(".. odin:autopackage:: core:fmt\n", &Config::default());
    assert!(codes.contains("rst.directive.unknown"), "{codes}");
    let recorder = Arc::new(Recorder::default());
    let config = Config {
        autodoc: vec![Autodoc { domain: "odin".to_owned(), provider: recorder.clone() }],
        ..Config::default()
    };
    expect_contains_with(
        ".. odin:package:: core:fmt\n\n.. odin:autoproc:: println\n   :members:\n",
        &["@id=\"odin-core-fmt.made\""],
        &config,
    );
    let seen = recorder.seen.lock().map(|s| s.clone()).unwrap_or_default();
    assert_eq!(seen, ("odin:autoproc".to_owned(), "println".to_owned(), "core:fmt".to_owned()));
}

/// A private type a package lists stays text; its public types still link.
#[test]
fn private_types_stay_text() {
    let text = ".. odin:package:: box\n   :private: Mode State\n\n\
        .. odin:struct:: Box :: struct\n\n   .. odin:field:: state: State\n\n\
        \x20  .. odin:field:: next: ^Box\n";
    expect_contains(
        text,
        &["text \"State\"", "xref link=unresolved name=\"Box\" @reftype=\"odin:type\""],
    );
    let got = read_text(text);
    assert!(!got.contains("name=\"State\""), "{got}");
}

/// A procedure group's private members stay text; its public ones link.
#[test]
fn private_group_members_stay_text() {
    let got = read_text(
        ".. odin:package:: box\n   :private: show_box\n\n\
         .. odin:procgroup:: show :: proc {show_box, show_int}\n",
    );
    assert!(!got.contains("name=\"show_box\""), "{got}");
    assert!(got.contains("text \"show_box\""), "{got}");
    assert!(got.contains("name=\"show_int\" @reftype=\"odin:proc\""), "{got}");
}
