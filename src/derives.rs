// Shared identity and helper metadata. A sort key is present only for derives
// covered by grouped-derive-order; API path replacement also accepts the others.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Derive {
    pub(crate) name: &'static str,
    pub(crate) key: Option<(usize, usize)>,
    pub(crate) krate: Option<&'static str>,
    pub(crate) helpers: &'static [&'static str],
}

impl Derive {
    const fn builtin(name: &'static str, key: (usize, usize)) -> Self {
        Self {
            name,
            key: Some(key),
            krate: None,
            helpers: &[],
        }
    }

    const fn external(
        krate: &'static str,
        name: &'static str,
        helpers: &'static [&'static str],
        key: Option<(usize, usize)>,
    ) -> Self {
        Self {
            name,
            key,
            krate: Some(krate),
            helpers,
        }
    }

    pub(crate) fn matches(&self, name: &str, krate: &str, builtin: bool) -> bool {
        self.name == name
            && match self.krate {
                None => builtin,
                Some(expected) => krate == expected,
            }
    }
}

pub(crate) fn resolve(name: &str, krate: &str, builtin: bool) -> Option<&'static Derive> {
    DERIVES.iter().find(|derive| derive.matches(name, krate, builtin))
}

pub(crate) fn helpers(name: &str, krate: &str, builtin: bool) -> Option<&'static [&'static str]> {
    if builtin {
        Some(&[])
    } else {
        resolve(name, krate, false).map(|derive| derive.helpers)
    }
}

pub(crate) const DERIVES: &[Derive] = &[
    Derive::builtin("Debug", (0, 0)),
    Derive::external("derive_more_impl", "Debug", &["debug"], Some((0, 0))),
    Derive::external("derive_more_impl", "Display", &["display"], Some((0, 1))),
    Derive::external("strum_macros", "Display", &["strum"], Some((0, 1))),
    Derive::external("derive_more_impl", "Error", &["error"], Some((0, 2))),
    Derive::external(
        "thiserror_impl",
        "Error",
        &["error", "from", "source", "backtrace"],
        Some((0, 2)),
    ),
    Derive::builtin("Clone", (1, 0)),
    Derive::builtin("Copy", (1, 1)),
    Derive::builtin("Default", (2, 0)),
    Derive::external("derive_more_impl", "Constructor", &[], Some((2, 1))),
    Derive::builtin("PartialEq", (3, 0)),
    Derive::external("derive_more_impl", "PartialEq", &["partial_eq"], Some((3, 0))),
    Derive::builtin("Eq", (3, 1)),
    Derive::external("derive_more_impl", "Eq", &["eq"], Some((3, 1))),
    Derive::builtin("PartialOrd", (3, 2)),
    Derive::builtin("Ord", (3, 3)),
    Derive::builtin("Hash", (3, 4)),
    Derive::external("serde_derive", "Serialize", &["serde"], Some((4, 0))),
    Derive::external("serde_repr", "Serialize_repr", &[], Some((4, 1))),
    Derive::external("serde_derive", "Deserialize", &["serde"], Some((4, 2))),
    Derive::external("serde_repr", "Deserialize_repr", &["serde"], Some((4, 3))),
    Derive::external("is_empty_derive", "IsEmpty", &["is_empty"], Some((5, 0))),
    Derive::external("derive_more_impl", "IsVariant", &["is_variant"], Some((5, 1))),
    Derive::external("strum_macros", "EnumIs", &["strum"], Some((5, 2))),
    Derive::external("strum_macros", "EnumCount", &["strum"], Some((6, 0))),
    Derive::external("strum_macros", "VariantNames", &["strum"], Some((6, 1))),
    Derive::external("strum_macros", "VariantArray", &["strum"], Some((6, 2))),
    Derive::external("strum_macros", "EnumProperty", &["strum"], Some((6, 3))),
    Derive::external("strum_macros", "EnumMessage", &["strum"], Some((6, 4))),
    Derive::external("derive_more_impl", "From", &["from"], Some((7, 0))),
    Derive::external("derive_more_impl", "TryFrom", &["try_from"], Some((7, 1))),
    Derive::external("derive_more_impl", "FromStr", &["from_str"], Some((7, 2))),
    Derive::external("strum_macros", "EnumString", &["strum"], Some((7, 3))),
    Derive::external("strum_macros", "FromRepr", &["strum"], Some((7, 4))),
    Derive::external("derive_more_impl", "AsRef", &["as_ref"], Some((8, 0))),
    Derive::external("strum_macros", "AsRefStr", &["strum"], Some((8, 1))),
    Derive::external("derive_more_impl", "AsMut", &["as_mut"], Some((8, 2))),
    Derive::external("derive_more_impl", "Deref", &["deref"], Some((8, 3))),
    Derive::external("derive_more_impl", "DerefMut", &["deref_mut"], Some((8, 4))),
    Derive::external("derive_more_impl", "Index", &["index"], Some((8, 5))),
    Derive::external("derive_more_impl", "IndexMut", &["index_mut"], Some((8, 6))),
    Derive::external("derive_more_impl", "Unwrap", &["unwrap"], Some((9, 0))),
    Derive::external("derive_more_impl", "TryUnwrap", &["try_unwrap"], Some((9, 1))),
    Derive::external("strum_macros", "EnumTryAs", &["strum"], Some((9, 2))),
    Derive::external("derive_more_impl", "IntoIterator", &["into_iterator"], Some((10, 0))),
    Derive::external("strum_macros", "EnumIter", &["strum"], Some((10, 1))),
    Derive::external("derive_more_impl", "Add", &["add"], None),
    Derive::external("derive_more_impl", "Sub", &["sub"], None),
    Derive::external("derive_more_impl", "BitAnd", &["bitand"], None),
    Derive::external("derive_more_impl", "BitOr", &["bitor"], None),
    Derive::external("derive_more_impl", "BitXor", &["bitxor"], None),
    Derive::external("derive_more_impl", "AddAssign", &["add_assign"], None),
    Derive::external("derive_more_impl", "SubAssign", &["sub_assign"], None),
    Derive::external("derive_more_impl", "BitAndAssign", &["bitand_assign"], None),
    Derive::external("derive_more_impl", "BitOrAssign", &["bitor_assign"], None),
    Derive::external("derive_more_impl", "BitXorAssign", &["bitxor_assign"], None),
    Derive::external("derive_more_impl", "Binary", &["binary"], None),
    Derive::external("derive_more_impl", "Octal", &["octal"], None),
    Derive::external("derive_more_impl", "LowerHex", &["lower_hex"], None),
    Derive::external("derive_more_impl", "UpperHex", &["upper_hex"], None),
    Derive::external("derive_more_impl", "LowerExp", &["lower_exp"], None),
    Derive::external("derive_more_impl", "UpperExp", &["upper_exp"], None),
    Derive::external("derive_more_impl", "Pointer", &["pointer"], None),
    Derive::external("derive_more_impl", "Into", &["into"], None),
    Derive::external("derive_more_impl", "Mul", &["mul"], None),
    Derive::external("derive_more_impl", "Div", &["div"], None),
    Derive::external("derive_more_impl", "Rem", &["rem"], None),
    Derive::external("derive_more_impl", "Shr", &["shr"], None),
    Derive::external("derive_more_impl", "Shl", &["shl"], None),
    Derive::external("derive_more_impl", "MulAssign", &["mul_assign"], None),
    Derive::external("derive_more_impl", "DivAssign", &["div_assign"], None),
    Derive::external("derive_more_impl", "RemAssign", &["rem_assign"], None),
    Derive::external("derive_more_impl", "ShrAssign", &["shr_assign"], None),
    Derive::external("derive_more_impl", "ShlAssign", &["shl_assign"], None),
    Derive::external("derive_more_impl", "Not", &[], None),
    Derive::external("derive_more_impl", "Neg", &[], None),
    Derive::external("derive_more_impl", "Sum", &[], None),
    Derive::external("derive_more_impl", "Product", &[], None),
    Derive::external("derive_more_impl", "TryInto", &["try_into"], None),
    Derive::external("strum_macros", "EnumVariantNames", &["strum"], None),
    Derive::external("strum_macros", "AsStaticStr", &["strum"], None),
    Derive::external("strum_macros", "IntoStaticStr", &["strum"], None),
    Derive::external("strum_macros", "ToString", &["strum"], None),
    Derive::external("strum_macros", "EnumTable", &["strum"], None),
    Derive::external(
        "strum_macros",
        "EnumDiscriminants",
        &["strum", "strum_discriminants"],
        None,
    ),
];

#[cfg(test)]
mod tests {
    use super::{
        helpers,
        resolve,
    };

    #[test]
    fn helpers_follow_the_exact_derive_identity() {
        assert_eq!(helpers("Debug", "core", true), Some(&[][..]));
        assert_eq!(helpers("Debug", "derive_more_impl", false), Some(&["debug"][..]));
        assert_eq!(helpers("Display", "strum_macros", false), Some(&["strum"][..]));
        assert_eq!(helpers("Debug", "strum_macros", false), None);
        assert_eq!(
            helpers("Error", "thiserror_impl", false),
            Some(&["error", "from", "source", "backtrace"][..])
        );

        for (derive, helper) in [("Add", "add"), ("SubAssign", "sub_assign"), ("ShrAssign", "shr_assign")] {
            let registered = resolve(derive, "derive_more_impl", false).unwrap_or_else(|| panic!("missing {derive}"));
            assert_eq!(registered.helpers, &[helper]);
            // Recognition for path replacement does not opt arithmetic derives into sorting.
            assert!(registered.key.is_none());
        }
    }
}
