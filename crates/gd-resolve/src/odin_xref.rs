//! References to objects of the `odin` domain.

use crate::lookups::Look;
use crate::xref::Resolved;

impl Look<'_, '_> {
    /// Finds an Odin object as the odin domain names it (`<package>.<name>`): in the
    /// type and package the reference is written in, then as written; then by the end
    /// of its name after a package path's `:` or `/`, so `fmt.println` finds
    /// `core:fmt.println`, as Odin code names packages by their last segment; and, for
    /// a leading `.`, by any suffix.
    pub(crate) fn resolve_odin(&mut self, reftype: &str) -> Option<Resolved> {
        let q = self.query;
        let name = q.target.trim_start_matches(['~', '.', '!']);
        let package = q.context("odin:package");
        let typ = q.context("odin:type");
        let mut candidates = Vec::new();
        if !package.is_empty() && !typ.is_empty() {
            candidates.push(format!("{package}.{typ}.{name}"));
        }
        if !package.is_empty() {
            candidates.push(format!("{package}.{name}"));
        }
        candidates.push(name.to_string());
        for c in candidates {
            if let Some(o) = self.object(&format!("odin:{c}")) {
                return Some(Look::at(o));
            }
        }
        for sep in [":", "/"] {
            if let Some(o) = self.suffix("odin:", &format!("{sep}{name}"), reftype) {
                return Some(Look::at(o));
            }
        }
        if q.refspecific || q.target.starts_with('.') {
            return self.suffix("odin:", &format!(".{name}"), reftype).map(Look::at);
        }
        None
    }
}
