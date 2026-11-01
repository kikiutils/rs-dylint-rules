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

fn resolved_derive(path: &str) -> Option<&'static Derive> {
    if let Some((prefix, name)) = path.split_once("::") {
        let krate = match prefix {
            "derive_more" => "derive_more_impl",
            "strum" => "strum_macros",
            _ => return None,
        };

        DERIVES.iter().find(|derive| derive.matches(name, krate, false))
    } else {
        DERIVES.iter().find(|derive| derive.name == path)
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
fn helpers_require_the_resolved_identity_not_a_shared_name_or_sort_key() {
    for (path, helper, ordered) in [
        ("Debug", "debug", None),
        ("derive_more::Debug", "debug", Some("derive_more::Debug, Clone, Hash")),
        ("Eq", "eq", None),
        ("derive_more::Eq", "eq", Some("Clone, derive_more::Eq, Hash")),
        ("PartialEq", "partial_eq", None),
        (
            "derive_more::PartialEq",
            "partial_eq",
            Some("Clone, derive_more::PartialEq, Hash"),
        ),
        ("strum::Display", "display", None),
        (
            "derive_more::Display",
            "display",
            Some("derive_more::Display, Clone, Hash"),
        ),
        ("strum::Display", "strum", Some("strum::Display, Clone, Hash")),
        ("derive_more::Display", "strum", None),
    ] {
        for attribute in [
            format!("{helper}(unused)"),
            format!("cfg_attr(all(), {helper}(unused))"),
        ] {
            let derives = format!("Hash, {path}, Clone");
            let source = format!("#[{attribute}] #[derive({derives})] struct Value;");
            let root = parse(&source);
            let output = analysis::analyze(&root, &source, |range| resolved_derive(&source[range]));
            assert_eq!(output.issues.len(), 1, "{source}");
            let expected = ordered.map_or_else(|| source.clone(), |ordered| source.replace(&derives, ordered));
            assert_eq!(apply(&source, &output), expected, "{source}");
        }
    }
}

#[test]
fn matching_checks_name_and_source_before_selecting_an_entry() {
    for (name, krate, builtin, expected) in [
        ("Debug", "core", true, Some(("Debug", None))),
        (
            "Debug",
            "derive_more_impl",
            false,
            Some(("Debug", Some("derive_more_impl"))),
        ),
        (
            "Display",
            "derive_more_impl",
            false,
            Some(("Display", Some("derive_more_impl"))),
        ),
        (
            "Display",
            "strum_macros",
            false,
            Some(("Display", Some("strum_macros"))),
        ),
        ("Debug", "strum_macros", false, None),
        ("Debug", "core", false, None),
        ("Other", "derive_more_impl", false, None),
    ] {
        let found = DERIVES
            .iter()
            .find(|derive| derive.matches(name, krate, builtin))
            .map(|derive| (derive.name, derive.krate));

        assert_eq!(found, expected, "{name} from {krate}, builtin={builtin}");
    }
}

#[test]
fn shared_sort_keys_preserve_order_and_do_not_repeat_names_in_messages() {
    let source = "#[derive(Eq, strum::Display, derive_more::Display, PartialEq)] struct Value;";
    let root = parse(source);
    let output = analysis::analyze(&root, source, |range| resolved_derive(&source[range]));
    assert_eq!(
        apply(source, &output),
        "#[derive(strum::Display, derive_more::Display, PartialEq, Eq)] struct Value;"
    );

    assert_eq!(
        analysis::groups_message(DERIVES),
        "Debug, Display, Error; Clone, Copy; Default, Constructor; PartialEq, Eq, PartialOrd, Ord, Hash; Serialize, Serialize_repr, Deserialize, Deserialize_repr; IsEmpty, IsVariant, EnumIs; EnumCount, VariantNames, VariantArray, EnumProperty, EnumMessage; From, TryFrom, FromStr, EnumString, FromRepr; AsRef, AsRefStr, AsMut, Deref, DerefMut, Index, IndexMut; Unwrap, TryUnwrap, EnumTryAs; IntoIterator, EnumIter"
    );
}
