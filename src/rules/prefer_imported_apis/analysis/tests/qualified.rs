use ra_ap_syntax::{
    AstNode,
    ast,
};
use rustc_hir::def::Namespace;

use super::*;

#[test]
fn fallback_and_fix_reuse_the_nearest_visible_import() {
    let source = "use std::io;\npub fn run() { use std::io; let _: std::io::Result<()> = Ok(()); }\n";
    let root = parse(source);
    let local = root
        .descendants()
        .find_map(ast::StmtList::cast)
        .unwrap_or_else(|| panic!("missing local scope"));

    let qualifier = qualifier("std::io", "io", false);
    let mut facts: Vec<_> = ranges(source, "io;")
        .map(|range| {
            (
                TextRange::new(range.start(), range.end() - TextSize::from(1)),
                Event::NamespaceImport {
                    qualifier: qualifier.clone(),
                    name: "io".to_owned(),
                    renamed: false,
                },
            )
        })
        .collect();

    facts.push((
        range(source, "std::io::Result"),
        Event::Use {
            api: api(
                "Result",
                Namespace::TypeNS,
                Form::Qualified {
                    qualifier,
                    member: "Result".to_owned(),
                },
            ),
            kind: UseKind::Path,
        },
    ));

    let output = super::super::analyze(
        &root,
        source,
        ra_ap_syntax::Edition::Edition2024,
        facts
            .iter()
            .map(|(range, event)| (*range, rustc_hir::CRATE_HIR_ID, event)),
        &super::super::super::fix::Checks {
            name: &|request| request.name != "io" || request.target == local.syntax().text_range(),
            remove: &|_| true,
            reference: &|_| false,
            depends: &|_| false,
            protected: &crate::syntax::protected,
        },
    );

    assert_eq!(output.issues.len(), 1);
    assert_eq!(apply(source, &output), source.replace("std::io::Result", "io::Result"));
}

#[test]
fn qualified_external_types_keep_generic_and_associated_suffixes() {
    let source = "fn run() { let _: serde_json::Value = serde_json::Value::Null; }\n";
    let api = api(
        "Value",
        Namespace::TypeNS,
        Form::Qualified {
            qualifier: qualifier("serde_json", "serde_json", true),
            member: "Value".to_owned(),
        },
    );

    let facts = [(
        range(source, "serde_json::Value"),
        Event::Use {
            kind: UseKind::Path,
            api,
        },
    )];

    assert_eq!(analyze(source, &facts, true).issues, []);
}

#[test]
fn noncanonical_namespace_aliases_shorten_only_the_resolved_api_prefix() {
    let source = "fn run() { let _: ::std::io::Result<u8> = Ok(0); }\n";
    let api = api(
        "Result",
        Namespace::TypeNS,
        Form::Qualified {
            qualifier: qualifier("std::io", "io", false),
            member: "Result".to_owned(),
        },
    );

    let facts = [(
        range(source, "::std::io::Result"),
        Event::Use {
            kind: UseKind::Path,
            api,
        },
    )];

    let fixed = apply(source, &analyze(source, &facts, true));
    assert_eq!(fixed, "fn run() { use std::io;\nlet _: io::Result<u8> = Ok(0); }\n");
}

#[test]
fn aliases_of_unmanaged_namespace_members_do_not_diagnose_the_import() {
    let source = "use std::process as proc;\nfn run() { proc::Command::new(\"x\"); }\n";
    let import_range = range(source, "proc;");
    let facts = [(
        TextRange::new(import_range.start(), import_range.end() - TextSize::from(1)),
        Event::NamespaceImport {
            qualifier: qualifier("std::process", "process", false),
            name: "proc".to_owned(),
            renamed: true,
        },
    )];

    assert_eq!(analyze(source, &facts, true).issues, []);
}

#[test]
fn extern_crate_bindings_obey_redundant_alias_and_import_order_rules() {
    for (declaration, renamed, late) in [
        ("extern crate anyhow;", false, false),
        ("extern crate anyhow as anyhow;", true, false),
        ("extern crate anyhow;", false, true),
    ] {
        let function = "fn run() { let _: Option<anyhow::Error> = None; }";
        let source = if late {
            format!("{function}\n{declaration}\n")
        } else {
            format!("{declaration}\n{function}\n")
        };

        let qualifier = qualifier("anyhow", "anyhow", true);
        let api = api(
            "Error",
            Namespace::TypeNS,
            Form::Qualified {
                qualifier: qualifier.clone(),
                member: "Error".to_owned(),
            },
        );

        let facts = [
            (
                range(&source, declaration),
                Event::NamespaceImport {
                    qualifier,
                    name: "anyhow".to_owned(),
                    renamed,
                },
            ),
            (
                range(&source, "anyhow::Error"),
                Event::Use {
                    kind: UseKind::Path,
                    api,
                },
            ),
        ];

        let output = analyze(&source, &facts, true);
        assert_eq!(output.issues.is_empty(), !renamed && !late, "{source}");
        assert_eq!(apply(&source, &output), source);
    }
}
