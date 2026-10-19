use ra_ap_syntax::{
    AstNode,
    Edition,
    SourceFile,
};

use super::analysis;
use crate::diagnostic::RuleOutput;

fn check(source: &str) -> RuleOutput {
    let parsed = SourceFile::parse(source, Edition::Edition2024);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    analysis::analyze(parsed.tree().syntax(), |range| {
        let name = &source[std::ops::Range::<usize>::from(range)];
        analysis::sort_key(name)
    })
}

fn apply(source: &str, output: &RuleOutput) -> String {
    let mut edits: Vec<_> = output
        .issues
        .iter()
        .filter_map(|issue| issue.fix.as_ref())
        .flat_map(|fix| &fix.edits)
        .collect();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
    let mut fixed = source.to_owned();
    for edit in edits {
        fixed.replace_range(std::ops::Range::<usize>::from(edit.range), &edit.replacement);
    }
    fixed
}

#[test]
fn groups_builtin_derives_and_is_idempotent() {
    let source = "#[derive(Hash, Ord, Eq, Copy, PartialOrd, Default, Clone, Debug, PartialEq)]\nstruct Value;\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 1);
    let fixed = apply(source, &output);
    assert_eq!(
        fixed,
        "#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]\nstruct Value;\n"
    );
    assert!(check(&fixed).issues.is_empty(), "{:?}", check(&fixed));
}

#[test]
fn separate_attributes_and_unknown_lists_remain_independent() {
    let source = "#[derive(Eq, PartialEq)]\n#[derive(Copy, Clone)]\n#[derive(Eq, Unknown, PartialEq)]\nstruct Value;\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 2);
    assert_eq!(
        apply(source, &output),
        "#[derive(PartialEq, Eq)]\n#[derive(Clone, Copy)]\n#[derive(Eq, Unknown, PartialEq)]\nstruct Value;\n"
    );
}

#[test]
fn comments_and_other_enclosing_attributes_block_fixes() {
    for source in [
        "#[derive(Eq, /* attached */ PartialEq)]\nstruct Value;\n",
        "#[observe]\n#[derive(Eq, PartialEq)]\nstruct Value;\n",
        "#![observe]\n#[derive(Eq, PartialEq)]\nstruct Value;\n",
        "#[observe]\nmod nested { #[derive(Eq, PartialEq)] struct Value; }\n",
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
        "#[derive(Unknown, Eq, PartialEq)] struct Value;\n",
    ] {
        assert!(check(source).issues.is_empty(), "{source}");
    }
}

#[test]
fn preserves_path_spelling_trailing_comma_and_raw_crlf_offsets() {
    let source =
        "const LABEL: &str = \"型別\";\r\n#[derive(core::cmp::Eq, core::cmp::PartialEq,)]\r\nstruct Value;\r\n";
    let parsed = SourceFile::parse(source, Edition::Edition2024);
    let output = analysis::analyze(parsed.tree().syntax(), |range| {
        let name = source[std::ops::Range::<usize>::from(range)].rsplit("::").next()?;
        analysis::sort_key(name)
    });
    assert_eq!(
        apply(source, &output),
        "const LABEL: &str = \"型別\";\r\n#[derive(core::cmp::PartialEq, core::cmp::Eq,)]\r\nstruct Value;\r\n"
    );
}

#[test]
fn serialization_group_follows_builtins_and_keeps_its_own_fixed_order() {
    let source = "#[derive(Deserialize, Hash, Serialize, Eq, PartialEq, Debug)]\nstruct Value;\n";
    assert_eq!(
        apply(source, &check(source)),
        "#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]\nstruct Value;\n"
    );
}

#[test]
fn builtin_attributes_and_resolved_helpers_allow_fixes() {
    for attribute in [
        "#[repr(u8)]",
        "#[repr(C, u8)]",
        "#[repr(/* note */ u8)]",
        "#[serde(rename_all = \"camelCase\")]",
    ] {
        let source = format!(
            "#![allow(dead_code)]\n#[cfg(all())]\nmod nested {{ {attribute}\n#[derive(Deserialize, Serialize, Debug)] struct Value; }}\n"
        );
        assert_eq!(
            apply(&source, &check(&source)),
            source.replace("Deserialize, Serialize, Debug", "Debug, Serialize, Deserialize")
        );
    }
    for (helper, derives) in [("is_empty", "IsEmpty, Debug"), ("strum", "EnumIs, Debug")] {
        let source = format!("#[{helper}(unused)] #[derive({derives})] struct Value;");
        assert_ne!(apply(&source, &check(&source)), source);
    }
}

#[test]
fn helpers_require_a_matching_derive_on_the_same_item() {
    for source in [
        "#[serde(other)] #[derive(Eq, PartialEq)] struct Value;",
        "#[serde(other)] mod nested { #[derive(Deserialize, Serialize)] struct Value; }",
        "#[cfg_attr(all(), observe)] #[derive(Eq, PartialEq)] struct Value;",
    ] {
        let output = check(source);
        assert_eq!(output.issues.len(), 1);
        assert!(output.issues[0].fix.is_none());
    }
    let source = "#[serde(rename_all = \"camelCase\")] #[derive(Serialize)] #[derive(Eq, PartialEq)] struct Value;";
    assert_eq!(
        apply(source, &check(source)),
        source.replace("Eq, PartialEq", "PartialEq, Eq")
    );
}
