mod analysis;
pub(super) mod collect;

rustc_lint_defs::declare_lint! { pub KIKI_GROUPED_DERIVE_ORDER, Warn, "group supported derives in a consistent order" }

struct Derive {
    name: &'static str,
    key: analysis::SortKey,
    // None for builtin derives; otherwise the defining proc-macro crate.
    krate: Option<&'static str>,
    helper: Option<&'static str>,
    check_automatically_derived: bool,
}

const DERIVES: &[Derive] = &[
    Derive {
        name: "Debug",
        key: (0, 0),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Clone",
        key: (1, 0),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Copy",
        key: (1, 1),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Default",
        key: (2, 0),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "PartialEq",
        key: (3, 0),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Eq",
        key: (3, 1),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "PartialOrd",
        key: (3, 2),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Ord",
        key: (3, 3),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Hash",
        key: (3, 4),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Serialize",
        key: (4, 0),
        krate: Some("serde_derive"),
        helper: Some("serde"),
        check_automatically_derived: true,
    },
    Derive {
        name: "Serialize_repr",
        key: (4, 1),
        krate: Some("serde_repr"),
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Deserialize",
        key: (4, 2),
        krate: Some("serde_derive"),
        helper: Some("serde"),
        check_automatically_derived: true,
    },
    Derive {
        name: "Deserialize_repr",
        key: (4, 3),
        krate: Some("serde_repr"),
        helper: Some("serde"),
        check_automatically_derived: false,
    },
    Derive {
        name: "IsEmpty",
        key: (5, 0),
        krate: Some("is_empty_derive"),
        helper: Some("is_empty"),
        check_automatically_derived: true,
    },
    Derive {
        name: "EnumIs",
        key: (5, 1),
        krate: Some("strum_macros"),
        helper: Some("strum"),
        check_automatically_derived: true,
    },
    Derive {
        name: "EnumCount",
        key: (6, 0),
        krate: Some("strum_macros"),
        helper: Some("strum"),
        check_automatically_derived: true,
    },
    Derive {
        name: "VariantNames",
        key: (6, 1),
        krate: Some("strum_macros"),
        helper: Some("strum"),
        check_automatically_derived: true,
    },
    Derive {
        name: "VariantArray",
        key: (6, 2),
        krate: Some("strum_macros"),
        helper: Some("strum"),
        check_automatically_derived: true,
    },
];

#[cfg(test)]
mod tests;
