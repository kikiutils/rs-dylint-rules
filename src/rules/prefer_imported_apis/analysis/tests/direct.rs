use rustc_hir::def::Namespace;

use super::*;

#[test]
fn direct_calls_keep_turbofish_and_batch_their_new_binding() {
    let source = "fn run() { tokio::time::sleep::<u8>(); tokio::time::sleep::<u16>(); }\n";
    let event = Event::Use {
        kind: UseKind::Path,
        api: api("sleep", Namespace::ValueNS, Form::Direct),
    };

    let facts: Vec<_> = ranges(source, "tokio::time::sleep")
        .map(|range| (range, event.clone()))
        .collect();

    let output = analyze(source, &facts, true);
    let fixed = apply(source, &output);
    assert_eq!(output.issues.len(), 2);
    assert_eq!(
        fixed,
        "use tokio::time::sleep;\nfn run() { sleep::<u8>(); sleep::<u16>(); }\n"
    );
}

#[test]
fn newly_inserted_outer_imports_and_nested_calls_share_one_suggestion() {
    let source = "fn run() { tokio::time::sleep(); { tokio::time::sleep(); } }\n";
    let event = Event::Use {
        kind: UseKind::Path,
        api: api("sleep", Namespace::ValueNS, Form::Direct),
    };

    let facts: Vec<_> = ranges(source, "tokio::time::sleep")
        .map(|range| (range, event.clone()))
        .collect();

    let output = analyze(source, &facts, true);
    let fixes: Vec<_> = output.issues.iter().filter_map(|issue| issue.fix.as_ref()).collect();
    assert_eq!(fixes.len(), 1);
    assert_eq!(
        apply(source, &output),
        "use tokio::time::sleep;\nfn run() { sleep(); { sleep(); } }\n"
    );
}

#[test]
fn macro_binding_and_dependent_calls_share_one_suggestion() {
    let source = "fn run() { anyhow::bail!(\"a\"); anyhow::bail!(\"b\"); }\n";
    let event = Event::Use {
        kind: UseKind::Macro,
        api: api("bail", Namespace::MacroNS, Form::Direct),
    };

    let facts = [
        (range(source, "anyhow::bail!(\"a\")"), event.clone()),
        (range(source, "anyhow::bail!(\"b\")"), event),
    ];

    let output = analyze(source, &facts, true);
    let fixes: Vec<_> = output.issues.iter().filter_map(|issue| issue.fix.as_ref()).collect();
    assert_eq!(fixes.len(), 1);
    assert_eq!(
        apply(source, &output),
        "use anyhow::bail;\nfn run() { bail!(\"a\"); bail!(\"b\"); }\n"
    );
}

#[test]
fn context_methods_accept_imports_declared_after_the_use() {
    let source = "fn run() { value.context(\"error\"); }\nuse anyhow::Context;\n";
    let api = api("Context", Namespace::TypeNS, Form::Direct);
    let facts = [
        (
            range(source, "Context"),
            Event::Import {
                api: api.clone(),
                name: "Context".to_owned(),
                renamed: false,
            },
        ),
        (
            range(source, "context"),
            Event::Use {
                kind: UseKind::Trait,
                api,
            },
        ),
    ];

    let output = analyze(source, &facts, true);
    assert_eq!(output.issues, []);
    assert_eq!(apply(source, &output), source);
}

#[test]
fn split_and_raw_direct_paths_never_introduce_a_redundant_alias() {
    for path in ["tokio::time::\n    sleep", "tokio::time::r#sleep"] {
        let source = format!("fn run() {{ {path}(duration); }}\n");
        let facts = [(
            range(&source, path),
            Event::Use {
                kind: UseKind::Path,
                api: api("sleep", Namespace::ValueNS, Form::Direct),
            },
        )];

        let fixed = apply(&source, &analyze(&source, &facts, true));
        assert_eq!(fixed, "use tokio::time::sleep;\nfn run() { sleep(duration); }\n");
    }
}
