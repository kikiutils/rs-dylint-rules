mod analysis;
pub(super) mod collect;
mod fix;
mod model;
mod names;
mod policy;

rustc_lint_defs::declare_lint! {
    pub KIKI_PREFER_IMPORTED_APIS,
    Warn,
    "use selected APIs with their prescribed imports and namespaces"
}
