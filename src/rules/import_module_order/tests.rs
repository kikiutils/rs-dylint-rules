use ra_ap_syntax::{
    AstNode,
    ast,
};

use super::analysis::{
    self,
    Group,
};
use crate::{
    diagnostic::RuleOutput,
    test_support::{
        apply,
        parse,
    },
};

fn check(source: &str) -> RuleOutput {
    let root = parse(source);
    analysis::analyze(
        &root,
        &|path: &ast::Path| {
            matches!(path.syntax().text().to_string().as_str(), "std" | "core" | "anyhow").then_some(Group::External)
        },
        &|_| true,
    )
}

#[test]
fn orders_groups_without_changing_import_tokens() {
    let source = "use crate::{A as Alias, nested::*};\nmod child;\nuse std::{io, fmt};\n";
    let output = check(source);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(output.issues[0].diagnostics.len(), 2);
    assert_eq!(
        apply(source, &output),
        "use std::{io, fmt};\n\nmod child;\n\nuse crate::{A as Alias, nested::*};\n"
    );
}

#[test]
fn reorders_use_declarations_with_comments_without_changing_their_text() {
    for (source, expected) in [
        (
            "use crate::{/* local */ A};\nmod child;\nuse std::{fmt, /* external */ io};\n",
            "use std::{fmt, /* external */ io};\n\nmod child;\n\nuse crate::{/* local */ A};\n",
        ),
        (
            "use crate::{nested::{A as Alias, /* nested */ *}};\nuse std::{\n    // about fmt\n    fmt,\n};\n",
            "use std::{\n    // about fmt\n    fmt,\n};\n\nuse crate::{nested::{A as Alias, /* nested */ *}};\n",
        ),
        (
            "use crate /* local path */ ::A;\nuse std /* external path */ ::fmt;\n",
            "use std /* external path */ ::fmt;\n\nuse crate /* local path */ ::A;\n",
        ),
        (
            "use /* local */ crate::A;\nuse /* external */ std::fmt;\n",
            "use /* external */ std::fmt;\n\nuse /* local */ crate::A;\n",
        ),
        (
            "use crate::A /* local */;\nuse std::fmt /* external */;\n",
            "use std::fmt /* external */;\n\nuse crate::A /* local */;\n",
        ),
        (
            "pub /* local */ use crate::A;\npub(crate /* external */) use std::fmt;\n",
            "pub(crate /* external */) use std::fmt;\n\npub /* local */ use crate::A;\n",
        ),
        (
            "use // local\ncrate::A;\nuse std::fmt // external\n;\n",
            "use std::fmt // external\n;\n\nuse // local\ncrate::A;\n",
        ),
    ] {
        let output = check(source);
        assert_eq!(output.issues.len(), 1, "{source}");
        let fixed = apply(source, &output);
        assert_eq!(fixed, expected);
        assert_eq!(check(&fixed), RuleOutput::default());
    }
}

#[test]
fn known_enclosing_attributes_and_same_line_declarations_allow_edits() {
    for prefix in [
        "",
        "#![feature(rustc_private)]",
        "#[inline]",
        "#[rustfmt::skip]",
        "#[clippy::msrv = \"1.90.0\"]",
    ] {
        let source = format!("{prefix}\nfn run() {{ use crate::A; use std::fmt; }}\n");
        let output = check(&source);
        assert_eq!(output.issues.len(), 1, "{source}");
        assert_eq!(
            apply(&source, &output),
            source.replace("use crate::A; use std::fmt;", "use std::fmt;\n\nuse crate::A;"),
            "{source}"
        );
    }
}

#[test]
fn only_test_required_modules_are_excluded() {
    for (predicate, excluded) in [
        ("test", true),
        ("all(test, unix)", true),
        ("all(unix, all(test, feature = \"x\"))", true),
        ("any(test, unix)", false),
        ("not(test)", false),
        ("feature = \"test\"", false),
    ] {
        let source = format!("use crate::A;\n#[cfg({predicate})] mod tests;\n");
        assert_eq!(check(&source).issues.is_empty(), excluded, "{source}");
    }
}

#[test]
fn comments_unknown_attributes_and_intervening_items_block_edits() {
    for source in [
        "use crate::A;\n// attached\nuse std::io;\n",
        "use crate::A; // trailing\nuse std::io;\n",
        "// leading\nuse crate::A;\nuse std::io;\n",
        "use crate::A;\nuse std::io; // edge\n",
        "use crate::A;\n/// documented import\nuse std::io;\n",
        "/* leading */ use crate::A;\nuse std::io;\n",
        "/* leading */ pub use crate::A;\npub use std::io;\n",
        "// leading\npub use crate::A;\npub use std::io;\n",
        "use crate::A;\n/* attached */ use std::io;\n",
        "use crate::A; /* trailing */\nuse std::io;\n",
        "use crate::A;\nuse std::io; /* edge */\n",
        "use crate::A;\nmod /* module */ child;\nuse std::fmt;\n",
        "use crate::A;\nfn unrelated() {}\nuse std::io;\n",
        "#![cfg_attr(any(), token_sensitive)]\nuse crate::A;\nuse std::io;\n",
        "#[token_sensitive] fn run() { use crate::A; use std::fmt; }\n",
        "use crate::A;\n#[expect(unused_imports)] use std::io;\n",
        "use crate::A;\n#[macro_use] mod child;\nuse std::io;\n",
    ] {
        let output = check(source);
        assert!(!output.issues.is_empty(), "{source}");
        assert!(output.issues.iter().all(|issue| issue.fix.is_none()), "{source}");
        assert_eq!(apply(source, &output), source);
    }
}

#[test]
fn mixed_or_unresolved_origins_are_diagnostic_only() {
    for source in ["use {std::io, crate::A};\n", "use unresolved::Value;\n"] {
        let output = check(source);
        assert_eq!(output.issues.len(), 1);
        assert!(output.issues[0].fix.is_none());
    }
}

#[test]
fn inactive_scopes_and_macro_tokens_are_skipped() {
    let source = "use crate::A;\nuse std::fmt;\nmacro_rules! generated { () => { use crate::A; use std::fmt; } }";
    let root = parse(source);
    let output = analysis::analyze(&root, &|_| Some(Group::External), &|_| false);
    assert_eq!(output, RuleOutput::default());
    assert_eq!(
        check("macro_rules! generated { () => { use crate::A; use std::fmt; } }"),
        RuleOutput::default()
    );
}

#[test]
fn cfg_and_lint_attributes_move_with_their_declarations() {
    let source = "use crate::A;\n#[cfg(unix)] mod linux;\n#[cfg(windows)] mod windows;\n#[allow(unused_imports)] use std::{/* inner */ io};\n";
    let root = parse(source);
    let output = analysis::analyze(
        &root,
        &|path| (path.syntax().text() == "std").then_some(Group::External),
        &|node| !node.text().to_string().starts_with("#[cfg(windows)]"),
    );

    assert_eq!(
        apply(source, &output),
        "#[allow(unused_imports)] use std::{/* inner */ io};\n\n#[cfg(unix)] mod linux;\n#[cfg(windows)] mod windows;\n\nuse crate::A;\n"
    );
}
