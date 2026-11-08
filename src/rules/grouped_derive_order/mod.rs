mod analysis;
pub(super) mod collect;

use crate::derives::{
    DERIVES,
    Derive,
};

rustc_lint_defs::declare_lint! { pub KIKI_GROUPED_DERIVE_ORDER, Warn, "group supported derives in a consistent order" }

#[cfg(test)]
mod tests;
