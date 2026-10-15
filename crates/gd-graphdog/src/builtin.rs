//! The built-in plugins of a static build (`cfg(graphviz_static)`, set by build.rs).
//!
//! A shared Graphviz finds its plugins at run time through the config file that `dot -c`
//! writes. A static one has no files to load, so the plugin libraries are linked in and
//! their tables handed to `gvContextPlugins`, as Graphviz's own `dot_static` does with
//! libtool's `lt_preloaded_symbols` (cmd/dot/dot_builtins.cpp). Each plugin library
//! exports one `gvplugin_library_t` named `gvplugin_<name>_LTX_library`.
//!
//! One more plugin is ours: a text-layout plugin. Graphviz sizes node shapes from text
//! widths, which a shared Graphviz measures with pango (real fonts). The static build has
//! no pango, so Graphviz falls back to width tables that cover ASCII only: every byte of
//! a UTF-8 character counts as a space, which makes CJK labels overflow their shapes, and
//! it warns about it. Our plugin keeps Graphviz's tables for ASCII text and for the ASCII
//! runs of mixed text, and sizes the other characters by their class (see [`RuneWidth`]).
//! Typst and browsers draw the SVG text with real fonts; the estimate only has to make
//! the shapes fit.

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::ptr;

use crate::ffi::LtSymlist;

/// Opaque `gvplugin_library_t` of a linked Graphviz plugin: only its address matters.
#[repr(C)]
struct PluginLibrary {
    _private: [u8; 0],
}

unsafe extern "C" {
    static gvplugin_dot_layout_LTX_library: PluginLibrary;
    static gvplugin_neato_layout_LTX_library: PluginLibrary;
    static gvplugin_core_LTX_library: PluginLibrary;

    /// lib/common/textspan_lut.h: Graphviz's built-in font metrics (Times, Helvetica,
    /// Courier and others), in points for a 1 pt font. Not public API, but the static
    /// build pins the Graphviz version, so it cannot move.
    fn estimate_text_width_1pt(
        font_name: *const c_char,
        text: *const c_char,
        bold: bool,
        italic: bool,
    ) -> f64;
}

/// `pointf` in lib/common/geom.h.
#[repr(C)]
struct Pointf {
    x: f64,
    y: f64,
}

/// `textfont_t` in lib/common/textspan.h (Graphviz 16.1, which the static build pins).
#[repr(C)]
struct Textfont {
    name: *mut c_char,
    color: *mut c_char,
    postscript_alias: *mut c_void,
    size: f64,
    /// Bit fields: the flags in the low 7 bits (`HTML_BF` = 1, `HTML_IF` = 2), then a
    /// reference count.
    bits: c_uint,
}

const HTML_BF: c_uint = 1;
const HTML_IF: c_uint = 2;

/// `textspan_t` in lib/common/textspan.h.
#[repr(C)]
struct Textspan {
    text: *mut c_char,
    font: *mut Textfont,
    layout: *mut c_void,
    free_layout: Option<unsafe extern "C" fn(*mut c_void)>,
    yoffset_layout: f64,
    yoffset_centerline: f64,
    size: Pointf,
    just: c_char,
}

/// `gvtextlayout_engine_t` in gvplugin_textlayout.h.
#[repr(C)]
struct TextlayoutEngine {
    textlayout: unsafe extern "C" fn(span: *mut Textspan, fontpath: *mut *mut c_char) -> bool,
}

/// `gvplugin_installed_t` in gvplugin.h.
#[repr(C)]
struct PluginInstalled {
    id: c_int,
    kind: *const c_char,
    quality: c_int,
    engine: *const c_void,
    features: *const c_void,
}

/// `gvplugin_api_t` in gvplugin.h.
#[repr(C)]
struct PluginApi {
    api: c_int,
    types: *const PluginInstalled,
}

/// `gvplugin_library_t` in gvplugin.h.
#[repr(C)]
struct PluginLibraryTable {
    packagename: *const c_char,
    apis: *const PluginApi,
}

/// `API_textlayout` in the `api_t` enum of gvcext.h.
const API_TEXTLAYOUT: c_int = 2;

/// A table of C pointers that Graphviz only reads.
#[repr(transparent)]
pub(crate) struct Table<T>(T);

// SAFETY: every table holds pointers to immutable statics (string literals, other
// tables, functions); neither Rust nor Graphviz writes through them, so sharing them
// between threads is sound.
unsafe impl<T> Sync for Table<T> {}

static TEXTLAYOUT_ENGINE: TextlayoutEngine = TextlayoutEngine { textlayout: estimate_span };

static TEXTLAYOUT_TYPES: Table<[PluginInstalled; 2]> = Table([
    PluginInstalled {
        id: 0,
        kind: c"textlayout".as_ptr(),
        quality: 0,
        engine: (&raw const TEXTLAYOUT_ENGINE).cast(),
        features: ptr::null(),
    },
    PluginInstalled {
        id: 0,
        kind: ptr::null(),
        quality: 0,
        engine: ptr::null(),
        features: ptr::null(),
    },
]);

static TEXTLAYOUT_APIS: Table<[PluginApi; 2]> = Table([
    PluginApi { api: API_TEXTLAYOUT, types: (&raw const TEXTLAYOUT_TYPES).cast() },
    PluginApi { api: 0, types: ptr::null() },
]);

static GRAPHDOG_LIBRARY: Table<PluginLibraryTable> = Table(PluginLibraryTable {
    packagename: c"graphdog".as_ptr(),
    apis: (&raw const TEXTLAYOUT_APIS).cast(),
});

/// The plugin table for `gvContextPlugins`, ended by a null entry.
static BUILTINS: Table<[LtSymlist; 5]> = Table([
    LtSymlist {
        name: c"gvplugin_dot_layout_LTX_library".as_ptr(),
        address: (&raw const gvplugin_dot_layout_LTX_library).cast(),
    },
    LtSymlist {
        name: c"gvplugin_neato_layout_LTX_library".as_ptr(),
        address: (&raw const gvplugin_neato_layout_LTX_library).cast(),
    },
    LtSymlist {
        name: c"gvplugin_core_LTX_library".as_ptr(),
        address: (&raw const gvplugin_core_LTX_library).cast(),
    },
    LtSymlist {
        name: c"gvplugin_graphdog_LTX_library".as_ptr(),
        address: (&raw const GRAPHDOG_LIBRARY).cast(),
    },
    LtSymlist { name: ptr::null(), address: ptr::null() },
]);

/// The built-in plugin table, which lives for the whole program.
pub(crate) fn builtins() -> *const LtSymlist {
    (&raw const BUILTINS).cast()
}

/// How wide a character is, compared with Latin letters of the same font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuneWidth {
    /// Combining marks, joiners and variation selectors.
    Zero,
    /// Lower-case and caseless letters and digits: like `o`.
    Lower,
    /// Upper-case letters: like `N`.
    Upper,
    /// Arrows, math and other symbols: like `M`.
    Symbol,
    /// East Asian wide and fullwidth characters, and emoji: one em.
    Wide,
}

/// Classifies a non-ASCII character for width estimates.
pub(crate) fn rune_width(c: char) -> RuneWidth {
    match u32::from(c) {
        0x200B..=0x200F | 0x2060..=0x2064 | 0xFE00..=0xFE0F | 0xE0100..=0xE01EF => {
            return RuneWidth::Zero;
        }
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD => return RuneWidth::Wide,
        _ => {}
    }
    if is_combining(c) {
        RuneWidth::Zero
    } else if c.is_uppercase() {
        RuneWidth::Upper
    } else if c.is_alphanumeric() {
        RuneWidth::Lower
    } else {
        RuneWidth::Symbol
    }
}

/// Whether `c` is in one of the blocks of combining marks. The standard library has no
/// general categories; the blocks cover the marks that labels meet in practice.
fn is_combining(c: char) -> bool {
    matches!(
        u32::from(c),
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

/// Sizes one span of text in points, as Graphviz's `estimate_textspan_size` does.
/// Returning false hands the span back to Graphviz's own estimate, which ASCII text gets
/// unchanged.
///
/// # Safety
///
/// Graphviz calls it with a valid span whose `text` and `font` (with its `name`) are
/// live, NUL-terminated where strings, and with `fontpath` null or writable.
unsafe extern "C" fn estimate_span(span: *mut Textspan, fontpath: *mut *mut c_char) -> bool {
    // SAFETY: Graphviz passes a live span and font for the call (see above).
    let (span, font) = unsafe { (&mut *span, &*(*span).font) };
    // SAFETY: the span's text is a live NUL-terminated string.
    let text = unsafe { CStr::from_ptr(span.text) };
    if text.to_bytes().is_ascii() {
        return false;
    }
    let bold = font.bits & HTML_BF != 0;
    let italic = font.bits & HTML_IF != 0;
    let width = width_1pt(font.name, &text.to_string_lossy(), bold, italic);
    span.size = Pointf { x: font.size * width, y: font.size * 1.2 };
    // LINESPACING and the offsets as in Graphviz's textspan.c.
    span.yoffset_layout = font.size;
    span.yoffset_centerline = 0.1 * font.size;
    span.layout = ptr::null_mut();
    span.free_layout = None;
    if !fontpath.is_null() {
        // SAFETY: a non-null `fontpath` is writable; Graphviz only prints the string,
        // as it does with the literal its own estimate stores.
        unsafe { *fontpath = c"[graphdog estimate]".as_ptr().cast_mut() };
    }
    true
}

/// Measures ASCII runs with Graphviz's tables and adds the other characters by class,
/// all for a 1 pt font.
fn width_1pt(font: *const c_char, text: &str, bold: bool, italic: bool) -> f64 {
    let measure = |run: &[u8]| -> f64 {
        let mut buffer = [0u8; 129];
        let len = run.len().min(128);
        buffer[..len].copy_from_slice(&run[..len]);
        // SAFETY: `buffer` is NUL-terminated (its last byte stays 0) and `font` is the
        // live font name Graphviz passed in.
        unsafe { estimate_text_width_1pt(font, buffer.as_ptr().cast(), bold, italic) }
    };
    let widths = |width: RuneWidth| match width {
        RuneWidth::Zero => 0.0,
        RuneWidth::Lower => measure(b"o"),
        RuneWidth::Upper => measure(b"N"),
        RuneWidth::Symbol => measure(b"M"),
        RuneWidth::Wide => 1.0,
    };
    let mut total = 0.0;
    let mut run = Vec::with_capacity(128);
    for c in text.chars() {
        if c.is_ascii() && run.len() < 128 {
            run.push(c as u8);
            continue;
        }
        total += measure(&run);
        run.clear();
        if c.is_ascii() {
            run.push(c as u8);
        } else {
            total += widths(rune_width(c));
        }
    }
    total + measure(&run)
}

#[cfg(test)]
mod tests {
    use super::{RuneWidth, rune_width};

    #[test]
    fn rune_widths_classify() {
        assert_eq!(rune_width('日'), RuneWidth::Wide);
        assert_eq!(rune_width('한'), RuneWidth::Wide);
        assert_eq!(rune_width('Ö'), RuneWidth::Upper);
        assert_eq!(rune_width('λ'), RuneWidth::Lower);
        assert_eq!(rune_width('\u{301}'), RuneWidth::Zero);
        assert_eq!(rune_width('→'), RuneWidth::Symbol);
    }
}
