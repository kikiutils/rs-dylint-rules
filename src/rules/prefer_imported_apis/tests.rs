use ra_ap_syntax::{
    TextRange,
    TextSize,
};

use super::{
    Api,
    analysis::{
        self,
        ApiUse,
        Kind,
    },
};
use crate::{
    diagnostic::RuleOutput,
    test_support::{
        apply,
        parse,
    },
};

fn check(source: &str, free: bool) -> RuleOutput {
    let root = parse(source);
    let fact = ApiUse {
        api: Api::Result,
        kind: Kind::Access,
    };
    let facts = source
        .match_indices("anyhow::Result")
        .map(|(start, path)| (TextRange::at(TextSize::of(&source[..start]), TextSize::of(path)), &fact));

    analysis::analyze(&root, source, facts, |_| free)
}

#[test]
fn qualified_access_is_fixed_only_when_the_name_is_free() {
    let source = "fn run() -> anyhow::Result<()> { Ok(()) }";
    for (free, expected) in [
        (true, "use anyhow::Result;\nfn run() -> Result<()> { Ok(()) }"),
        (false, source),
    ] {
        let output = check(source, free);
        assert_eq!(output.issues.len(), 1);
        assert_eq!(apply(source, &output), expected);
    }
}

#[test]
fn imports_follow_inner_docs_and_attributes_without_detaching_outer_docs() {
    for (source, expected) in [
        (
            "//! Root docs\nfn run() -> anyhow::Result<()> { Ok(()) }",
            "//! Root docs\nuse anyhow::Result;\nfn run() -> Result<()> { Ok(()) }",
        ),
        (
            "/*! Root docs */\n#![allow(dead_code)]\nfn run() -> anyhow::Result<()> { Ok(()) }",
            "/*! Root docs */\n#![allow(dead_code)]\nuse anyhow::Result;\nfn run() -> Result<()> { Ok(()) }",
        ),
        (
            "mod m {\n#![allow(dead_code)]\n//! Module docs\nfn run() -> anyhow::Result<()> { Ok(()) }\n}",
            "mod m {\n#![allow(dead_code)]\n//! Module docs\nuse anyhow::Result;\n\nfn run() -> Result<()> { Ok(()) }\n}",
        ),
        (
            "/// Function docs\nfn run() -> anyhow::Result<()> { Ok(()) }",
            "use anyhow::Result;\n/// Function docs\nfn run() -> Result<()> { Ok(()) }",
        ),
    ] {
        assert_eq!(apply(source, &check(source, true)), expected, "{source}");
    }

    let source =
        "//! Root docs\n// Keep preamble comment\n#![allow(dead_code)]\nfn run() -> anyhow::Result<()> { Ok(()) }";
    let output = check(source, true);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(apply(source, &output), source);
}

#[test]
fn planned_outer_imports_include_inner_replacements_in_the_same_fix() {
    let source = "fn run() -> anyhow::Result<()> { let _: anyhow::Result<()> = Ok(()); Ok(()) }";
    let output = check(source, true);
    assert_eq!(output.issues.len(), 2);
    assert_eq!(output.issues.iter().filter(|issue| issue.fix.is_some()).count(), 1);
    assert_eq!(
        apply(source, &output),
        "use anyhow::Result;\nfn run() -> Result<()> { let _: Result<()> = Ok(()); Ok(()) }"
    );
}
