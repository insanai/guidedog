//! The math domain: its numref role (equations are the math directive's and the :eq:
//! role's, which belong to the layer itself).

use super::{Domain, DomainDirective, RoleSpec, XrefFlags};

static ROLES: &[RoleSpec] = &[RoleSpec::xref("numref", XrefFlags::CODE)];

/// The math domain.
#[derive(Debug)]
pub(crate) struct MathDomain;

/// The value `DOMAINS` registers.
pub(crate) static DOMAIN: MathDomain = MathDomain;

impl Domain for MathDomain {
    fn name(&self) -> &'static str {
        "math"
    }

    fn directives(&self) -> &'static [DomainDirective] {
        &[]
    }

    fn roles(&self) -> &'static [RoleSpec] {
        ROLES
    }
}
