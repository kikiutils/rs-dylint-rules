use super::{
    super::super::policy::ImportTarget,
    *,
};

fn distinct(index: u32) -> DefId {
    DefId {
        index: rustc_span::def_id::DefIndex::from_u32(index),
        ..definition()
    }
}

#[test]
fn shared_insertion_anchors_package_only_successful_transactions() {
    let source = "fn run() { tokio::time::sleep(); tokio::time::timeout(); }\n";
    let facts: Vec<_> = ["sleep", "timeout"]
        .into_iter()
        .map(|name| {
            (
                range(source, &format!("tokio::time::{name}")),
                Event::Use {
                    kind: UseKind::Path,
                    api: api(name, Namespace::ValueNS, Form::Direct),
                },
            )
        })
        .collect();

    for block_sleep in [false, true] {
        let output = super::super::analyze(
            &parse(source),
            source,
            ra_ap_syntax::Edition::Edition2024,
            facts
                .iter()
                .map(|(range, event)| (*range, rustc_hir::CRATE_HIR_ID, event)),
            &super::super::super::fix::Checks {
                name: &|request| !block_sleep || request.name != "sleep",
                remove: &|_| true,
                reference: &|_| false,
                depends: &|_| false,
                protected: &crate::syntax::protected,
            },
        );

        assert_eq!(output.issues.iter().filter(|issue| issue.fix.is_some()).count(), 1);
        assert_eq!(output.issues[0].fix.is_some(), !block_sleep);
        assert_eq!(output.issues[1].fix.is_some(), block_sleep);
        let fixed = apply(source, &output);
        assert!(fixed.contains("use tokio::time::timeout;"), "{fixed}");
        assert_eq!(fixed.contains("use tokio::time::sleep;"), !block_sleep);
    }
}

#[test]
fn unrelated_bindings_with_different_anchors_have_separate_fixes() {
    let source = "fn first() { tokio::time::sleep(); }\nfn second() { tokio::time::timeout(); }\n";
    let facts: Vec<_> = ["sleep", "timeout"]
        .into_iter()
        .map(|name| {
            (
                range(source, &format!("tokio::time::{name}")),
                Event::Use {
                    kind: UseKind::Path,
                    api: api(name, Namespace::ValueNS, Form::Direct),
                },
            )
        })
        .collect();

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.iter().filter(|issue| issue.fix.is_some()).count(), 2);
    assert!(output.issues[0].fix.is_some());
    assert!(output.issues[1].fix.is_some());
}

#[test]
fn failed_outer_migration_does_not_block_an_independent_inner_import() {
    let source = "use std::result::Result; // import explanation\npub fn root() -> Result<(), u8> { Ok(()) }\nfn inner() { let _: std::result::Result<(), u8> = Ok(()); }\n";
    let result = api(
        "Result",
        Namespace::TypeNS,
        Form::Replace {
            target: Some(ImportTarget {
                source: "anyhow::Result",
                definition: Some(distinct(1)),
                root: Some(distinct(2)),
            }),
        },
    );

    let imported = range(source, "Result;");
    let facts = [
        (
            TextRange::new(imported.start(), imported.end() - TextSize::from(1)),
            Event::Import {
                api: result.clone(),
                name: "Result".to_owned(),
                renamed: false,
            },
        ),
        (
            range(source, "Result<(), u8>"),
            Event::Use {
                api: result.clone(),
                kind: UseKind::Path,
            },
        ),
        (
            range(source, "std::result::Result<(), u8>"),
            Event::Use {
                api: result,
                kind: UseKind::Path,
            },
        ),
    ];

    // Facts cover the API prefix, not its generic arguments.
    let facts: Vec<_> = facts
        .into_iter()
        .map(|(range, event)| {
            let range = if matches!(event, Event::Use { .. }) {
                TextRange::new(range.start(), range.end() - TextSize::of("<(), u8>"))
            } else {
                range
            };

            (range, event)
        })
        .collect();

    let output = analyze(source, &facts, true);
    let fixed = apply(source, &output);
    assert_eq!(output.issues.iter().filter(|issue| issue.fix.is_some()).count(), 1);
    assert!(
        fixed.starts_with("use std::result::Result; // import explanation\npub fn root() -> Result"),
        "{fixed}"
    );

    assert!(
        fixed.contains("fn inner() { use anyhow::Result;\nlet _: Result<(), u8>"),
        "{fixed}"
    );
}

#[test]
fn an_inner_import_migration_is_independent_of_a_failed_outer_binding() {
    let source = "use std::result::Result; // import explanation\nfn run() -> Result<(), u8> { { use std::result::Result; let _: Result<(), u8> = Ok(()); } Ok(()) }\n";
    let result = api(
        "Result",
        Namespace::TypeNS,
        Form::Replace {
            target: Some(ImportTarget {
                source: "anyhow::Result",
                definition: Some(distinct(1)),
                root: Some(distinct(2)),
            }),
        },
    );

    let facts: Vec<_> = ranges(source, "Result")
        .enumerate()
        .map(|(index, range)| {
            (
                range,
                if index == 0 || index == 2 {
                    Event::Import {
                        api: result.clone(),
                        name: "Result".to_owned(),
                        renamed: false,
                    }
                } else {
                    Event::Use {
                        api: result.clone(),
                        kind: UseKind::Path,
                    }
                },
            )
        })
        .collect();

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.iter().filter(|issue| issue.fix.is_some()).count(), 1);
    let fixed = apply(source, &output);
    assert!(
        fixed.starts_with("use std::result::Result; // import explanation\nfn run() -> Result"),
        "{fixed}"
    );

    assert!(fixed.contains("{ use anyhow::Result;\n let _: Result"), "{fixed}");
}

#[test]
fn edits_to_the_same_use_declaration_share_a_fix() {
    let source = "use std::{io::Error as IoError, process::id};\npub fn error() -> IoError { loop {} }\npub fn current() -> u32 { id() }\n";
    let error = api(
        "Error",
        Namespace::TypeNS,
        Form::Qualified {
            qualifier: qualifier("std::io", "io", false),
            member: "Error".to_owned(),
        },
    );

    let mut id = api(
        "id",
        Namespace::ValueNS,
        Form::Qualified {
            qualifier: qualifier("std::process", "process", false),
            member: "id".to_owned(),
        },
    );

    id.definition = distinct(1);
    let error_import = range(source, "IoError,");
    let id_import = range(source, "id}");
    let facts = [
        (
            TextRange::new(error_import.start(), error_import.end() - TextSize::from(1)),
            Event::Import {
                api: error.clone(),
                name: "IoError".to_owned(),
                renamed: true,
            },
        ),
        (
            TextRange::new(id_import.start(), id_import.end() - TextSize::from(1)),
            Event::Import {
                api: id.clone(),
                name: "id".to_owned(),
                renamed: false,
            },
        ),
        (
            range(source, "IoError {"),
            Event::Use {
                api: error,
                kind: UseKind::Path,
            },
        ),
        (
            range(source, "id()"),
            Event::Use {
                api: id,
                kind: UseKind::Path,
            },
        ),
    ];

    let facts: Vec<_> = facts
        .into_iter()
        .enumerate()
        .map(|(index, (range, event))| {
            let range = if index == 2 || index == 3 {
                TextRange::new(range.start(), range.end() - TextSize::from(2))
            } else {
                range
            };

            (range, event)
        })
        .collect();

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.iter().filter(|issue| issue.fix.is_some()).count(), 1);
    let fixed = apply(source, &output);
    assert!(!fixed.contains("use std::{"), "{fixed}");
    assert!(
        fixed.contains("io::Error") && fixed.contains("process::id()"),
        "{fixed}"
    );
}

#[test]
fn retained_import_routes_merge_their_provider_changes() {
    let source = "use anyhow as a;\npub fn error() -> a::Error { loop {} }\npub fn fail() { a::bail!(\"example\") }\n";
    let mut namespace = qualifier("anyhow", "anyhow", true);
    namespace.definition = distinct(10);
    let error = api(
        "Error",
        Namespace::TypeNS,
        Form::Qualified {
            qualifier: namespace.clone(),
            member: "Error".to_owned(),
        },
    );

    let mut bail = api("bail", Namespace::MacroNS, Form::Direct);
    bail.definition = distinct(1);
    let imported = range(source, "a;");
    let facts = [
        (
            TextRange::new(imported.start(), imported.end() - TextSize::from(1)),
            Event::NamespaceImport {
                qualifier: namespace,
                name: "a".to_owned(),
                renamed: true,
            },
        ),
        (
            range(source, "a::Error"),
            Event::Use {
                api: error,
                kind: UseKind::Path,
            },
        ),
        (
            range(source, "a::bail!(\"example\")"),
            Event::Use {
                api: bail,
                kind: UseKind::Macro,
            },
        ),
    ];

    let output = super::super::analyze(
        &parse(source),
        source,
        ra_ap_syntax::Edition::Edition2024,
        facts
            .iter()
            .map(|(range, event)| (*range, rustc_hir::CRATE_HIR_ID, event)),
        &super::super::super::fix::Checks {
            name: &|_| true,
            remove: &|_| true,
            reference: &|_| false,
            depends: &|request| request.name == "a" && !request.retained.is_empty(),
            protected: &crate::syntax::protected,
        },
    );

    assert_eq!(output.issues.iter().filter(|issue| issue.fix.is_some()).count(), 1);
    let fixed = apply(source, &output);
    assert!(fixed.contains("use anyhow as a;"), "{fixed}");
    assert!(fixed.contains("use a::bail;"), "{fixed}");
    assert!(fixed.contains("-> anyhow::Error"), "{fixed}");
}
