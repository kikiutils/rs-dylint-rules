use rustc_hir::def::Namespace;

use super::*;

#[test]
fn glob_imports_with_multiple_managed_members_are_reported_once() {
    let source = "use std::process::*;\n";
    let import_range = range(source, "std::process");
    let facts: Vec<_> = ["id", "exit"]
        .into_iter()
        .map(|name| {
            (
                import_range,
                Event::Import {
                    api: api(
                        name,
                        Namespace::ValueNS,
                        Form::Qualified {
                            qualifier: qualifier("std::process", "process", false),
                            member: name.to_owned(),
                        },
                    ),
                    name: name.to_owned(),
                    renamed: false,
                },
            )
        })
        .collect();

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(output.issues[0].diagnostics[0].range, import_range);
    assert_eq!(apply(source, &output), source);
}

#[test]
fn repeated_canonical_accesses_report_a_redundant_namespace_import_once() {
    let source = "use std::process as process;\nfn run() { process::id(); process::id(); }\n";
    let qualifier = qualifier("std::process", "process", false);
    let alias_range = range(source, "process;");
    let mut facts = vec![(
        TextRange::new(alias_range.start(), alias_range.end() - TextSize::from(1)),
        Event::NamespaceImport {
            qualifier: qualifier.clone(),
            name: "process".to_owned(),
            renamed: true,
        },
    )];

    let api = api(
        "id",
        Namespace::ValueNS,
        Form::Qualified {
            qualifier,
            member: "id".to_owned(),
        },
    );

    facts.extend(ranges(source, "process::id").map(|range| {
        (
            range,
            Event::Use {
                kind: UseKind::Path,
                api: api.clone(),
            },
        )
    }));

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues.len(), 1);
    assert_eq!(
        output.issues[0].diagnostics[0].range,
        range(source, "std::process as process")
    );

    assert_eq!(apply(source, &output), source);
}
