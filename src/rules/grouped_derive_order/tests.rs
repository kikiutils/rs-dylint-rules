use super::{
    DERIVES,
    Derive,
    analysis,
};
use crate::{
    diagnostic::RuleOutput,
    test_support::{
        apply,
        parse,
    },
};

const COLLIDING_DERIVES: &[Derive] = &[
    Derive {
        name: "Debug",
        key: (0, 0),
        krate: None,
        helper: None,
        check_automatically_derived: false,
    },
    Derive {
        name: "Debug",
        key: (0, 0),
        krate: Some("first"),
        helper: Some("first"),
        check_automatically_derived: true,
    },
    Derive {
        name: "Debug",
        key: (0, 0),
        krate: Some("second"),
        helper: Some("second"),
        check_automatically_derived: false,
    },
];

fn resolved_derive(name: &str) -> Option<&'static Derive> {
    match name {
        "Debug" => Some(&COLLIDING_DERIVES[0]),
        "first::Debug" => Some(&COLLIDING_DERIVES[1]),
        "second::Debug" => Some(&COLLIDING_DERIVES[2]),
        _ => DERIVES.iter().find(|derive| derive.name == name),
    }
}

fn check(source: &str) -> RuleOutput {
    let root = parse(source);
    analysis::analyze(&root, source, |range| {
        let name = &source[range];
        DERIVES.iter().find(|derive| derive.name == name)
    })
}

#[test]
fn groups_builtin_derives_and_is_idempotent() {
    for (source, expected) in [
        (
            "#[derive(Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq)]\nstruct Value;\n",
            "#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]\nstruct Value;\n",
        ),
        (
            "#[derive(Deserialize, Hash, Serialize, Eq, PartialEq, Debug)]\nstruct Value;\n",
            "#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]\nstruct Value;\n",
        ),
    ] {
        let output = check(source);
        assert_eq!(output.issues.len(), 1);
        let fixed = apply(source, &output);
        assert_eq!(fixed, expected);
        assert_eq!(check(&fixed), RuleOutput::default());
    }
}

#[test]
fn separate_known_attributes_are_fixed_independently() {
    let source = "#[derive(Eq, PartialEq)]\n#[derive(Copy, Clone)]\nstruct Value;\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 2);
    assert_eq!(
        apply(source, &output),
        "#[derive(PartialEq, Eq)]\n#[derive(Clone, Copy)]\nstruct Value;\n"
    );
}

#[test]
fn comments_and_other_enclosing_attributes_block_fixes() {
    for source in [
        "#[derive(Eq, /* attached */ PartialEq)]\nstruct Value;\n",
        "#[observe]\n#[derive(Eq, PartialEq)]\nstruct Value;\n",
        "#![observe]\n#[derive(Eq, PartialEq)]\nstruct Value;\n",
        "#[observe]\nmod nested { #[derive(Eq, PartialEq)] struct Value; }\n",
        "#[derive(Unknown)] #[derive(Eq, PartialEq)] struct Value;",
        "#[derive(Eq, PartialEq)] #[derive(Unknown)] struct Value;",
        "#[derive(Eq, Unknown, PartialEq)] #[derive(Eq, PartialEq)] struct Value;",
        "#[derive(Unknown)] struct Outer([u8; { #[derive(Eq, PartialEq)] struct Inner; 0 }]);",
    ] {
        let output = check(source);
        assert_eq!(output.issues.len(), 1, "{source}");
        assert!(output.issues[0].fix.is_none(), "{source}");
        assert_eq!(apply(source, &output), source);
    }
}

#[test]
fn skips_cfg_attr_and_macro_token_trees() {
    for source in [
        "#[cfg_attr(feature = \"x\", derive(Eq, PartialEq))]\nstruct Value;\n",
        "macro_rules! make { () => { #[derive(Eq, PartialEq)] struct Value; } }\n",
    ] {
        assert!(check(source).issues.is_empty(), "{source}");
    }
}

#[test]
fn preserves_path_spelling_and_raw_crlf_offsets() {
    let source =
        "const LABEL: &str = \"型別\";\r\n#[derive(core::cmp::Eq, core::cmp::PartialEq,)]\r\nstruct Value;\r\n";

    let root = parse(source);
    let output = analysis::analyze(&root, source, |range| {
        let name = source[range].rsplit("::").next()?;
        DERIVES.iter().find(|derive| derive.name == name)
    });

    assert_eq!(
        apply(source, &output),
        "const LABEL: &str = \"型別\";\r\n#[derive(core::cmp::PartialEq, core::cmp::Eq)]\r\nstruct Value;\r\n"
    );
}

#[test]
fn builtin_attributes_and_resolved_helpers_allow_fixes() {
    for attribute in [
        "#[repr(u8)]",
        "#[repr(C, u8)]",
        "#[repr(/* note */ u8)]",
        "#[serde(rename_all = \"camelCase\")]",
        "#[cfg_attr(all(), serde(rename_all = \"camelCase\"))]",
        "#[cfg_attr(any(), serde(rename_all = \"camelCase\"))]",
        "#[cfg_attr(all(), allow(dead_code), cfg_attr(all(), serde(rename_all = \"camelCase\")))]",
    ] {
        let source = format!(
            "#![allow(dead_code)]\n#[cfg(all())]\nmod nested {{ {attribute}\n#[derive(Deserialize, Serialize, Debug)] struct Value; }}\n"
        );

        assert_eq!(
            apply(&source, &check(&source)),
            source.replace("Deserialize, Serialize, Debug", "Debug, Serialize, Deserialize")
        );
    }

    for (helper, derives, expected) in [
        ("is_empty", "IsEmpty, Debug", "Debug, IsEmpty"),
        ("strum", "EnumIs, Debug", "Debug, EnumIs"),
    ] {
        let source = format!("#[{helper}(unused)] #[derive({derives})] struct Value;");
        assert_eq!(apply(&source, &check(&source)), source.replace(derives, expected));
    }
}

#[test]
fn helpers_require_a_matching_derive_on_the_same_item() {
    for source in [
        "#[serde(other)] #[derive(Eq, PartialEq)] struct Value;",
        "#[serde(other)] mod nested { #[derive(Deserialize, Serialize)] struct Value; }",
        "#[cfg_attr(all(), observe)] #[derive(Eq, PartialEq)] struct Value;",
        "#[cfg_attr(all(), serde(other))] #[derive(Eq, PartialEq)] struct Value;",
        "#[cfg_attr(all(), serde(other))] mod nested { #[derive(Deserialize, Serialize)] struct Value; }",
        "#[cfg_attr(any(), serde(other), observe)] #[derive(Deserialize, Serialize)] struct Value;",
        "#[cfg_attr(all(), serde(other), cfg_attr(any(), observe))] #[derive(Deserialize, Serialize)] struct Value;",
        "#[cfg_attr(all(), serde(other), derive(Serialize))] #[derive(Eq, PartialEq)] struct Value;",
    ] {
        let output = check(source);
        assert_eq!(output.issues.len(), 1);
        assert!(output.issues[0].fix.is_none());
    }

    for helper in [
        "serde(rename_all = \"camelCase\")",
        "cfg_attr(all(), serde(rename_all = \"camelCase\"))",
    ] {
        let source = format!("#[{helper}] #[derive(Serialize)] #[derive(Eq, PartialEq)] struct Value;");
        assert_eq!(
            apply(&source, &check(&source)),
            source.replace("Eq, PartialEq", "PartialEq, Eq")
        );
    }
}

#[test]
fn same_named_builtin_derives_do_not_authorize_third_party_helpers() {
    for (path, krate, helper, can_fix) in [
        ("Debug", "core", "debug", false),
        ("Debug", "derive_more_impl", "debug", true),
        ("Eq", "core", "eq", false),
        ("Eq", "derive_more_impl", "eq", true),
        ("PartialEq", "core", "partial_eq", false),
        ("PartialEq", "derive_more_impl", "partial_eq", true),
        ("Display", "strum_macros", "display", false),
        ("Display", "derive_more_impl", "display", true),
        ("Display", "strum_macros", "strum", true),
        ("Display", "derive_more_impl", "strum", false),
    ] {
        for attribute in [
            format!("{helper}(unused)"),
            format!("cfg_attr(all(), {helper}(unused))"),
        ] {
            let source = format!("#[{attribute}] #[derive(Hash, {path}, Clone)] struct Value;");
            let root = parse(&source);
            let output = analysis::analyze(&root, &source, |range| {
                let name = &source[range];
                if name == path {
                    DERIVES
                        .iter()
                        .find(|derive| derive.matches(name, krate, krate == "core", true))
                } else {
                    DERIVES.iter().find(|derive| derive.matches(name, "core", true, true))
                }
            });

            assert_eq!(output.issues.len(), 1, "{source}");
            assert_eq!(output.issues[0].fix.is_some(), can_fix, "{source}");
        }
    }
}

#[test]
fn matching_checks_name_and_source_before_selecting_an_entry() {
    for (name, krate, builtin, automatically_derived, expected) in [
        ("Debug", "core", true, true, Some(0)),
        ("Debug", "first", false, true, Some(1)),
        ("Debug", "second", false, false, Some(2)),
        ("Debug", "first", false, false, None),
        ("Debug", "unknown", false, true, None),
        ("Debug", "core", false, true, None),
        ("Other", "first", false, true, None),
    ] {
        let found = COLLIDING_DERIVES
            .iter()
            .find(|derive| derive.matches(name, krate, builtin, automatically_derived));

        assert_eq!(found, expected.map(|index| &COLLIDING_DERIVES[index]));
    }
}

#[test]
fn helpers_require_the_resolved_identity_not_a_shared_sort_key() {
    for (path, helper, can_fix) in [
        ("Debug", "first", false),
        ("first::Debug", "first", true),
        ("second::Debug", "first", false),
        ("second::Debug", "second", true),
        ("first::Debug", "second", false),
    ] {
        let source = format!("#[{helper}(unused)] #[derive(Eq, {path}, PartialEq)] struct Value;");
        let root = parse(&source);
        let output = analysis::analyze(&root, &source, |range| resolved_derive(&source[range]));
        assert_eq!(output.issues.len(), 1, "{source}");
        assert_eq!(output.issues[0].fix.is_some(), can_fix, "{source}");
        let expected = if can_fix {
            source.replace(&format!("Eq, {path}, PartialEq"), &format!("{path}, PartialEq, Eq"))
        } else {
            source.clone()
        };

        assert_eq!(apply(&source, &output), expected);
    }
}

#[test]
fn shared_sort_keys_preserve_order_and_do_not_repeat_names_in_messages() {
    let source = "#[derive(Eq, second::Debug, first::Debug, PartialEq)] struct Value;";
    let root = parse(source);
    let output = analysis::analyze(&root, source, |range| resolved_derive(&source[range]));
    assert_eq!(
        apply(source, &output),
        "#[derive(second::Debug, first::Debug, PartialEq, Eq)] struct Value;"
    );

    assert_eq!(analysis::groups_message(COLLIDING_DERIVES), "Debug");
    assert_eq!(
        analysis::groups_message(DERIVES),
        "Debug, Display, Error; Clone, Copy; Default, Constructor; PartialEq, Eq, PartialOrd, Ord, Hash; Serialize, Serialize_repr, Deserialize, Deserialize_repr; IsEmpty, IsVariant, EnumIs; EnumCount, VariantNames, VariantArray, EnumProperty, EnumMessage; From, TryFrom, FromStr, EnumString, FromRepr; AsRef, AsRefStr, AsMut, Deref, DerefMut, Index, IndexMut; Unwrap, TryUnwrap, EnumTryAs; IntoIterator, EnumIter"
    );
}
