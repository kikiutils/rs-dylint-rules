mod analysis;
pub(super) mod collect;

rustc_lint_defs::declare_lint! { pub KIKI_GROUPED_DERIVE_ORDER, Warn, "group supported derives in a consistent order" }

#[derive(Debug, PartialEq, Eq)]
struct Derive {
    name: &'static str,
    key: analysis::SortKey,
    // None for builtin derives; otherwise the defining proc-macro crate.
    krate: Option<&'static str>,
    helper: Option<&'static str>,
}

impl Derive {
    fn matches(&self, name: &str, krate: &str, builtin: bool) -> bool {
        self.name == name
            && match self.krate {
                None => builtin,
                Some(expected) => krate == expected,
            }
    }
}

const DERIVES: &[Derive] = &[
    Derive {
        name: "Debug",
        key: (0, 0),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Debug",
        key: (0, 0),
        krate: Some("derive_more_impl"),
        helper: Some("debug"),
    },
    Derive {
        name: "Display",
        key: (0, 1),
        krate: Some("derive_more_impl"),
        helper: Some("display"),
    },
    Derive {
        name: "Display",
        key: (0, 1),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "Error",
        key: (0, 2),
        krate: Some("derive_more_impl"),
        helper: Some("error"),
    },
    Derive {
        name: "Clone",
        key: (1, 0),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Copy",
        key: (1, 1),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Default",
        key: (2, 0),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Constructor",
        key: (2, 1),
        krate: Some("derive_more_impl"),
        helper: None,
    },
    Derive {
        name: "PartialEq",
        key: (3, 0),
        krate: None,
        helper: None,
    },
    Derive {
        name: "PartialEq",
        key: (3, 0),
        krate: Some("derive_more_impl"),
        helper: Some("partial_eq"),
    },
    Derive {
        name: "Eq",
        key: (3, 1),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Eq",
        key: (3, 1),
        krate: Some("derive_more_impl"),
        helper: Some("eq"),
    },
    Derive {
        name: "PartialOrd",
        key: (3, 2),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Ord",
        key: (3, 3),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Hash",
        key: (3, 4),
        krate: None,
        helper: None,
    },
    Derive {
        name: "Serialize",
        key: (4, 0),
        krate: Some("serde_derive"),
        helper: Some("serde"),
    },
    Derive {
        name: "Serialize_repr",
        key: (4, 1),
        krate: Some("serde_repr"),
        helper: None,
    },
    Derive {
        name: "Deserialize",
        key: (4, 2),
        krate: Some("serde_derive"),
        helper: Some("serde"),
    },
    Derive {
        name: "Deserialize_repr",
        key: (4, 3),
        krate: Some("serde_repr"),
        helper: Some("serde"),
    },
    Derive {
        name: "IsEmpty",
        key: (5, 0),
        krate: Some("is_empty_derive"),
        helper: Some("is_empty"),
    },
    Derive {
        name: "IsVariant",
        key: (5, 1),
        krate: Some("derive_more_impl"),
        helper: Some("is_variant"),
    },
    Derive {
        name: "EnumIs",
        key: (5, 2),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "EnumCount",
        key: (6, 0),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "VariantNames",
        key: (6, 1),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "VariantArray",
        key: (6, 2),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "EnumProperty",
        key: (6, 3),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "EnumMessage",
        key: (6, 4),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "From",
        key: (7, 0),
        krate: Some("derive_more_impl"),
        helper: Some("from"),
    },
    Derive {
        name: "TryFrom",
        key: (7, 1),
        krate: Some("derive_more_impl"),
        helper: Some("try_from"),
    },
    Derive {
        name: "FromStr",
        key: (7, 2),
        krate: Some("derive_more_impl"),
        helper: Some("from_str"),
    },
    Derive {
        name: "EnumString",
        key: (7, 3),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "FromRepr",
        key: (7, 4),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "AsRef",
        key: (8, 0),
        krate: Some("derive_more_impl"),
        helper: Some("as_ref"),
    },
    Derive {
        name: "AsRefStr",
        key: (8, 1),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "AsMut",
        key: (8, 2),
        krate: Some("derive_more_impl"),
        helper: Some("as_mut"),
    },
    Derive {
        name: "Deref",
        key: (8, 3),
        krate: Some("derive_more_impl"),
        helper: Some("deref"),
    },
    Derive {
        name: "DerefMut",
        key: (8, 4),
        krate: Some("derive_more_impl"),
        helper: Some("deref_mut"),
    },
    Derive {
        name: "Index",
        key: (8, 5),
        krate: Some("derive_more_impl"),
        helper: Some("index"),
    },
    Derive {
        name: "IndexMut",
        key: (8, 6),
        krate: Some("derive_more_impl"),
        helper: Some("index_mut"),
    },
    Derive {
        name: "Unwrap",
        key: (9, 0),
        krate: Some("derive_more_impl"),
        helper: Some("unwrap"),
    },
    Derive {
        name: "TryUnwrap",
        key: (9, 1),
        krate: Some("derive_more_impl"),
        helper: Some("try_unwrap"),
    },
    Derive {
        name: "EnumTryAs",
        key: (9, 2),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
    Derive {
        name: "IntoIterator",
        key: (10, 0),
        krate: Some("derive_more_impl"),
        helper: Some("into_iterator"),
    },
    Derive {
        name: "EnumIter",
        key: (10, 1),
        krate: Some("strum_macros"),
        helper: Some("strum"),
    },
];

#[cfg(test)]
mod tests;
