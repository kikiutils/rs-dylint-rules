use expect_test::expect;
use ra_ap_syntax::{
    AstNode,
    Edition,
    SourceFile,
    ast,
};

use super::analysis::{
    Group,
    analyze,
};
use crate::diagnostic::{
    Diagnostic,
    Edit,
    RuleOutput,
};

struct Analysis {
    diagnostics: Vec<Diagnostic>,
    edits: Vec<Edit>,
    fixed: usize,
}

impl From<RuleOutput> for Analysis {
    fn from(output: RuleOutput) -> Self {
        let mut result = Self {
            diagnostics: Vec::new(),
            edits: Vec::new(),
            fixed: 0,
        };
        for issue in output.issues {
            if let Some(fix) = issue.fix {
                result.fixed += issue.diagnostics.len();
                result.edits.extend(fix.edits);
            } else {
                result.diagnostics.extend(issue.diagnostics);
            }
        }
        result
    }
}

impl Analysis {
    fn apply(&self, source: &str) -> String {
        let mut output = source.to_owned();
        let mut edits: Vec<_> = self.edits.iter().collect();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
        for edit in edits {
            output.replace_range(std::ops::Range::<usize>::from(edit.range), &edit.replacement);
        }
        output
    }
}

// Functions
fn check(source: &str) -> Analysis {
    let parsed = SourceFile::parse(source, Edition::Edition2024);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    analyze(parsed.tree().syntax(), &external, &|_| true).into()
}

fn external(path: &ast::Path) -> Option<Group> {
    let text = path.syntax().text().to_string();
    matches!(text.as_str(), "std" | "core" | "alloc" | "tokio" | "anyhow").then_some(Group::External)
}

#[test]
fn test_only_external_modules_are_not_part_of_import_ordering() {
    for source in [
        "use crate::A;\n#[cfg(test)]\nmod tests;\n",
        "use crate::A;\n#[cfg( /* test-only */ test )]\nmod test_support;\n",
        "mod inline {\n    use crate::A;\n    #[cfg(test)]\n    mod tests;\n}\n",
    ] {
        let analysis = check(source);
        assert!(analysis.diagnostics.is_empty(), "{source}");
        assert!(analysis.edits.is_empty(), "{source}");
    }
    for source in [
        "use crate::A;\nmod tests;\n",
        "use crate::A;\n#[cfg(not(test))]\nmod tests;\n",
        "use crate::A;\n#[cfg(feature = \"test\")]\nmod tests;\n",
        "use crate::A;\n#[cfg(test)]\nmod tests;\nuse std::fmt;\n",
    ] {
        let analysis = check(source);
        assert_eq!(analysis.diagnostics.len() + analysis.fixed, 1, "{source}");
    }
}

#[test]
fn leaves_valid_groups_and_rustfmt_internal_order_unchanged() {
    for source in [
        "use std::io;\nuse tokio::spawn;\nmod child;\nuse crate::A;\nuse self::B;\nuse super::C;\n",
        "use crate::A;\n",
        "mod child;\n",
        "use crate::A;\nuse self::B;\n",
        "use tokio::spawn;\nuse std::io;\n",
    ] {
        let analysis = check(source);
        assert_eq!(analysis.diagnostics, Vec::new());
        assert_eq!(analysis.edits, Vec::new());
        assert_eq!(analysis.apply(source), source);
    }
}

#[test]
fn sorts_three_groups_stably_and_is_idempotent() {
    let source = "use crate::A;\nmod z;\nuse tokio::spawn;\nmod a;\nuse self::B;\nuse std::io;\n";
    let analysis = check(source);
    assert_eq!(analysis.diagnostics, Vec::new());
    assert_eq!(analysis.fixed, 4);
    let fixed = analysis.apply(source);
    expect![[r"
        use tokio::spawn;
        use std::io;

        mod z;
        mod a;

        use crate::A;
        use self::B;
    "]]
    .assert_eq(&fixed);

    assert_eq!(check(&fixed).edits, Vec::new());
    assert_eq!(check(&fixed).diagnostics, Vec::new());
}

#[test]
fn ordinary_statements_and_comments_do_not_reset_order() {
    for source in [
        "fn run() {\n    use crate::A;\n    work();\n    use std::io;\n}\n",
        "use crate::A;\n// group\nuse std::io;\n",
        "use crate::A;\nfn helper() {}\nuse std::io;\n",
        "use crate::A;\nmacro_rules! helper { () => {} }\nuse std::io;\n",
    ] {
        let analysis = check(source);
        assert_eq!(analysis.diagnostics.len(), 1, "{source}");
        assert!(analysis.edits.is_empty(), "{source}");
        assert_eq!(analysis.apply(source), source);
    }
}

#[test]
fn checks_nested_scopes_independently() {
    let source = "use crate::Outer;\nmod inline {\n    use self::Inner;\n    use std::io;\n}\nfn run() {\n    use crate::A;\n    use std::fmt;\n    let task = async {\n        use super::B;\n        use tokio::spawn;\n    };\n    let closure = || {\n        use crate::C;\n        use core::mem;\n    };\n    if true {\n        use self::D;\n        use alloc::vec;\n    }\n}\n";
    let analysis = check(source);
    assert_eq!(analysis.diagnostics, Vec::new());
    assert_eq!(analysis.edits.len(), 5);
    let fixed = analysis.apply(source);
    assert_eq!(check(&fixed).edits, Vec::new());
    assert_eq!(check(&fixed).diagnostics, Vec::new());
    assert!(fixed.starts_with("use crate::Outer;\nmod inline"));
}

#[test]
fn inline_module_is_not_an_external_module_declaration() {
    let source = "use crate::A;\nmod inline {}\n";
    let analysis = check(source);
    assert_eq!(analysis.edits, Vec::new());
    assert_eq!(analysis.diagnostics, Vec::new());
}

#[test]
fn keeps_comments_attributes_and_documentation_untouched() {
    for source in [
        "// local import\nuse crate::A;\nuse std::io;\n",
        "use crate::A; // local import\nuse std::io;\n",
        "use crate::A;\nuse std::io; // external import\n",
        "/// local import\nuse crate::A;\nuse std::io;\n",
        "#[allow(unused_imports)]\nuse crate::A;\nuse std::io;\n",
        "#[cfg(feature = \"demo\")]\nuse crate::A;\nuse std::io;\n",
        "use crate::A;\n#[macro_use]\nmod child;\n",
        "use crate::A;\n#[path = \"other.rs\"]\nmod child;\n",
    ] {
        let analysis = check(source);
        assert!(!analysis.diagnostics.is_empty(), "{source}");
        assert!(analysis.edits.is_empty(), "{source}");
        assert_eq!(analysis.apply(source), source);
    }
}

#[test]
fn grouped_and_glob_imports_preserve_their_contents() {
    let source = "use crate::{A as Alias, nested::*};\nmod child;\nuse std::{io, fmt};\n";
    let analysis = check(source);
    let fixed = analysis.apply(source);
    assert_eq!(analysis.diagnostics, Vec::new());
    assert_eq!(
        fixed,
        "use std::{io, fmt};\n\nmod child;\n\nuse crate::{A as Alias, nested::*};\n"
    );
}

#[test]
fn mixed_grouped_imports_are_diagnostic_only() {
    let source = "use {std::io, crate::A};\nmod child;\n";
    let analysis = check(source);
    assert_eq!(analysis.diagnostics.len(), 1);
    assert!(analysis.diagnostics[0].message.contains("mixed"));
    assert_eq!(analysis.edits, Vec::new());
}

#[test]
fn unresolved_imports_are_not_guessed_to_be_external() {
    let source = "use crate::A;\nuse unknown::B;\nuse std::io;\n";
    let analysis = check(source);
    assert_eq!(analysis.diagnostics.len(), 2);
    assert_eq!(analysis.edits, Vec::new());
}

#[test]
fn semantic_resolver_can_identify_unprefixed_local_imports() {
    let source = "use local::A;\nmod child;\nuse ::tokio::spawn;\n";
    let parsed = SourceFile::parse(source, Edition::Edition2024);
    let analysis: Analysis = analyze(
        parsed.tree().syntax(),
        &|path| {
            if path.syntax().text() == "local" {
                Some(Group::Local)
            } else {
                Some(Group::External)
            }
        },
        &|_| true,
    )
    .into();

    assert_eq!(analysis.diagnostics, Vec::new());
    assert_eq!(analysis.edits.len(), 1);
    assert_eq!(
        analysis.apply(source),
        "use ::tokio::spawn;\n\nmod child;\n\nuse local::A;\n"
    );
}

#[test]
fn all_import_visibilities_follow_source_groups_and_preserve_tokens() {
    for visibility in [
        "",
        "pub ",
        "pub(crate) ",
        "pub(super) ",
        "pub(self) ",
        "pub(in crate::parent) ",
    ] {
        let source = format!("{visibility}use crate::A as Alias;\nmod child;\n{visibility}use std::fmt;\n");
        let expected = format!("{visibility}use std::fmt;\n\nmod child;\n\n{visibility}use crate::A as Alias;\n");
        let analysis = check(&source);
        assert_eq!(analysis.diagnostics, Vec::new());
        assert_eq!(analysis.edits.len(), 1);
        assert_eq!(analysis.apply(&source), expected);
        assert_eq!(check(&expected).edits, Vec::new());
    }
}

#[test]
fn visible_imports_keep_comment_attribute_and_mixed_source_safety_checks() {
    for source in [
        "pub use crate::A;\n// attached\npub use std::fmt;\n",
        "#[allow(unused)]\npub(crate) use crate::A;\npub use std::fmt;\n",
        "pub use {crate::A, std::fmt};\n",
    ] {
        let analysis = check(source);
        assert!(!analysis.diagnostics.is_empty(), "{source}");
        assert_eq!(analysis.edits, Vec::new());
        assert_eq!(analysis.apply(source), source);
    }
}

#[test]
fn does_not_parse_macro_token_trees_as_rust_scopes() {
    let analysis = check("macro_rules! generated { () => { use crate::A; use std::io; }; }\n");
    assert_eq!(analysis.edits, Vec::new());
    assert_eq!(analysis.diagnostics, Vec::new());
}

#[test]
fn preserves_crlf_and_unicode_offsets() {
    let source = "const LABEL: &str = \"模組\";\r\n\r\nuse crate::A;\r\nuse std::io;\r\n";
    let analysis = check(source);
    assert_eq!(analysis.diagnostics, Vec::new());
    assert_eq!(
        analysis.apply(source),
        "const LABEL: &str = \"模組\";\r\n\r\nuse std::io;\r\n\r\nuse crate::A;\r\n"
    );
}

#[test]
fn same_line_nested_items_are_diagnostic_only() {
    let source = "fn run() { use crate::A; use std::io; }\n";
    let analysis = check(source);
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.edits, Vec::new());
}

#[test]
fn enclosing_attributes_make_nested_scopes_diagnostic_only() {
    for source in [
        "#[token_sensitive]\nmod wrapped {\n    use crate::A;\n    mod child;\n}\n",
        "#[token_sensitive]\nfn wrapped() {\n    use crate::A;\n    use std::io;\n}\n",
        "#[cfg_attr(feature = \"demo\", token_sensitive)]\nmod wrapped {\n    use crate::A;\n    mod child;\n}\n",
        "#[token_sensitive]\nmod wrapped {\n    mod nested {\n        use crate::A;\n        mod child;\n    }\n}\n",
        "#[token_sensitive]\nfn wrapped() {\n    let task = async {\n        let closure = || {\n            use crate::A;\n            use std::io;\n        };\n    };\n}\n",
        "mod wrapped {\n    #![cfg_attr(feature = \"demo\", token_sensitive)]\n    use crate::A;\n    mod child;\n}\n",
        "#![cfg_attr(feature = \"demo\", token_sensitive)]\nuse crate::A;\nmod child;\n",
        "#[allow(unused_imports)]\nfn wrapped() {\n    use crate::A;\n    use std::io;\n}\n",
    ] {
        let analysis = check(source);
        assert_eq!(analysis.diagnostics.len(), 1, "{source}");
        assert_eq!(analysis.fixed, 0, "{source}");
        assert!(analysis.edits.is_empty(), "{source}");
        assert_eq!(analysis.apply(source), source);
    }
}

#[test]
fn attributed_siblings_do_not_disable_safe_fixes_elsewhere() {
    let protected = "#[token_sensitive]\nmod wrapped {\n    use crate::A;\n    mod child;\n}\n";
    let source = format!("{protected}mod plain {{\n    use crate::A;\n    mod child;\n}}\n");
    let analysis = check(&source);
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.fixed, 1);
    assert_eq!(analysis.edits.len(), 1);
    let fixed = analysis.apply(&source);
    assert_eq!(
        fixed,
        format!("{protected}mod plain {{\n    mod child;\n\n    use crate::A;\n}}\n")
    );

    assert_eq!(check(&fixed).edits, Vec::new());
}
