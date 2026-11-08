use rustc_hir::def::Namespace;

use super::*;

fn process_use(source: &str) -> [(TextRange, Event); 1] {
    [(
        range(source, "std::process::id"),
        Event::Use {
            kind: UseKind::Path,
            api: api(
                "id",
                Namespace::ValueNS,
                Form::Qualified {
                    qualifier: qualifier("std::process", "process", false),
                    member: "id".to_owned(),
                },
            ),
        },
    )]
}

#[test]
fn imports_follow_inner_docs_and_inner_attributes() {
    for (source, expected) in [
        (
            "//! Crate documentation.\n#![allow(dead_code)]\nfn run() { std::process::id(); }\n",
            "//! Crate documentation.\n#![allow(dead_code)]use std::process;\n\nfn run() { process::id(); }\n",
        ),
        (
            "#![allow(dead_code)]\n//! Crate documentation.\nfn run() { std::process::id(); }\n",
            "#![allow(dead_code)]\n//! Crate documentation.\nuse std::process;\nfn run() { process::id(); }\n",
        ),
        (
            "mod child {\n//! Module documentation.\nfn run() { std::process::id(); }\n}\n",
            "mod child {\n//! Module documentation.\nuse std::process;\nfn run() { process::id(); }\n}\n",
        ),
        (
            "/*! Crate documentation. */fn run() { std::process::id(); }\n",
            "/*! Crate documentation. */use std::process;\nfn run() { process::id(); }\n",
        ),
        (
            "#!/usr/bin/env rust-script\nfn run() { std::process::id(); }\n",
            "#!/usr/bin/env rust-script\nuse std::process;\nfn run() { process::id(); }\n",
        ),
    ] {
        let output = analyze(source, &process_use(source), true);
        assert_eq!(apply(source, &output), expected, "{source}");
    }
}
