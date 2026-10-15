//! The interface for dialects (Sphinx, MyST hosts) that add roles.

use guidedoc::{ReadContext, SourceSpan};

/// One interpreted text offered to a [`RoleHook`]. The strings borrow the parser's
/// buffers and live only during the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleCall<'c> {
    /// The role name as written, or the default role's name.
    pub name: &'c str,
    /// The content with every escaping backslash as NUL and line breaks as "\n"; see
    /// [`unescape`].
    pub marked: &'c str,
    /// The content with escapes processed; line breaks become spaces.
    pub text: &'c str,
    /// The whole interpreted text, role included.
    pub span: SourceSpan,
    /// The content between the backquotes.
    pub content: SourceSpan,
}

/// A dialect's roles, consulted before the document's and the standard roles. `role`
/// emits nodes for the call through `ctx.b` (as children of the open node) and returns
/// true, or returns false for a role it does not know, and the standard lookup
/// continues. A closure `FnMut(&mut ReadContext, &RoleCall) -> bool` is a hook.
pub trait RoleHook {
    /// Handles `call`; false means "not mine".
    fn role(&mut self, ctx: &mut ReadContext<'_>, call: &RoleCall<'_>) -> bool;
}

impl<F> RoleHook for F
where
    F: FnMut(&mut ReadContext<'_>, &RoleCall<'_>) -> bool,
{
    fn role(&mut self, ctx: &mut ReadContext<'_>, call: &RoleCall<'_>) -> bool {
        self(ctx, call)
    }
}

/// The hook of plain reStructuredText: it knows no role.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRoleHook;

impl RoleHook for NoRoleHook {
    fn role(&mut self, _ctx: &mut ReadContext<'_>, _call: &RoleCall<'_>) -> bool {
        false
    }
}

/// Appends marked text (escapes as NUL, see [`RoleCall::marked`]) to `out` with escapes
/// processed: an escaped space or line break disappears, any other escaped character
/// stays, and an unescaped line break becomes a space.
pub fn unescape(out: &mut String, marked: &str) {
    let bytes = marked.as_bytes();
    let mut run = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c != 0 && c != b'\n' {
            i += 1;
            continue;
        }
        out.push_str(&marked[run..i]);
        i += 1;
        if c == b'\n' {
            out.push(' ');
        } else if i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
            i += 1;
        }
        run = i;
    }
    out.push_str(&marked[run..]);
}
