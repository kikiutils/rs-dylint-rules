mod analysis;
pub(super) mod collect;
mod fix;

rustc_lint_defs::declare_lint! { pub KIKI_IMPORT_MODULE_ORDER, Warn, "order imports and external module declarations" }

#[cfg(test)]
mod tests;
