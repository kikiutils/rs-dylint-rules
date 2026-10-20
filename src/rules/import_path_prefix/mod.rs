mod analysis;
pub(super) mod collect;

rustc_lint_defs::declare_lint! { pub KIKI_IMPORT_PATH_PREFIX, Warn, "use module-relative prefixes for imports within the current crate" }

#[cfg(test)]
mod tests;
