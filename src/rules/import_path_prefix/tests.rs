use ra_ap_syntax::{
    TextRange,
    TextSize,
};

use super::analysis::{
    self,
    Plan,
    Segment,
};
use crate::{
    diagnostic::Edit,
    test_support::parse,
};

fn segments(source: &str, names: &[&str]) -> Vec<Segment> {
    let mut offset = 0;
    names
        .iter()
        .map(|name| {
            let Some(relative) = source[offset..].find(name) else {
                panic!("missing test segment")
            };

            let start = offset + relative;
            offset = start + name.len();
            Segment {
                name: (*name).to_owned(),
                range: TextRange::new(TextSize::of(&source[..start]), TextSize::of(&source[..offset])),
            }
        })
        .collect()
}

#[test]
fn prefixes_use_the_closest_non_root_ancestor() {
    let source = "use crate::a::b::Value;";
    let segments = segments(source, &["crate", "a", "b", "Value"]);
    for (base, consumed, end, prefix) in [(2, 3, 15, "self"), (1, 2, 12, "super")] {
        assert_eq!(
            analysis::plan(&segments, 2, base, consumed),
            Some(Plan {
                range: TextRange::new(4.into(), end.into()),
                replacement: prefix.to_owned(),
            })
        );
    }

    assert_eq!(analysis::plan(&segments, 2, 0, 1), None);

    let source = "use super::super::other::Value;";
    let segments = self::segments(source, &["super", "super", "other", "Value"]);
    assert_eq!(
        analysis::plan(&segments, 2, 0, 2),
        Some(Plan {
            range: TextRange::new(4.into(), 16.into()),
            replacement: "crate".to_owned(),
        })
    );
}

#[test]
fn bare_paths_insert_the_prefix_at_the_first_segment() {
    let source = "// 型別\r\nuse child::Value;";
    let segments = segments(source, &["child", "Value"]);
    assert_eq!(
        analysis::plan(&segments, 2, 2, 0),
        Some(Plan {
            range: TextRange::empty(TextSize::of("// 型別\r\nuse ")),
            replacement: "self::".to_owned(),
        })
    );
}

#[test]
fn comments_groups_and_enclosing_attributes_block_unsafe_edits() {
    let edit_allowed = |source: &str, names: &[&str]| {
        let root = parse(source);
        let segments = segments(source, names);
        let Some(plan) = analysis::plan(&segments, 2, 2, 3) else {
            panic!("missing plan")
        };

        analysis::edit(&root, source, &segments, &plan, false).is_some()
    };

    for (source, allowed) in [
        ("use crate::a::b::Value;", true),
        ("use crate /*keep*/ ::a::b::Value;", false),
        ("use crate::{a::b::Value};", false),
        ("#[observe] mod m { use crate::a::b::Value; }", false),
        ("#[cfg(test)] mod m { use crate::a::b::Value; }", true),
        ("#[cfg(windows)] fn run() { use crate::a::b::Value; }", true),
        (
            "#![allow(dead_code)] #[cfg(all())] mod m { use crate::a::b::Value; }",
            true,
        ),
        ("#[cfg_attr(all(), observe)] mod m { use crate::a::b::Value; }", false),
        ("#[path = \"other.rs\"] mod m { use crate::a::b::Value; }", true),
    ] {
        assert_eq!(edit_allowed(source, &["crate", "a", "b", "Value"]), allowed, "{source}");
    }

    // Importing the anchor module itself needs a grouped self import instead.
    assert!(!edit_allowed("use crate::a::b as alias;", &["crate", "a", "b"]));
}

#[test]
fn cfg_attr_checks_all_nested_attributes() {
    for (attrs, allowed) in [
        ("#![cfg_attr(test, allow(dead_code))]", true),
        (
            "#[cfg_attr(all(), cfg_attr(test, allow(dead_code), doc = \"note\"))]",
            true,
        ),
        ("#[cfg_attr(any(), allow(dead_code), observe)]", false),
        ("#[cfg_attr(any(), cfg_attr(any(), observe))]", false),
    ] {
        let source = format!("{attrs} mod m {{ use crate::a::Value; }}");
        let root = parse(&source);
        let segments = segments(&source, &["crate", "a", "Value"]);
        let Some(plan) = analysis::plan(&segments, 1, 1, 2) else {
            panic!("missing plan")
        };

        assert_eq!(
            analysis::edit(&root, &source, &segments, &plan, false).is_some(),
            allowed,
            "{source}"
        );
    }
}

#[test]
fn split_preserves_visibility_attributes_aliases_and_globs() {
    let source = "#[cfg(all())] pub(crate) use crate::{a::{self as A, Value as V, *}, b::Other};";
    let root = parse(source);
    let segments = segments(source, &["crate", "a", "Value"]);
    let Some(edit) = analysis::split_import(&root, source, &segments, &[]) else {
        panic!("missing split edit")
    };

    assert_eq!(
        edit.replacement,
        "#[cfg(all())] pub(crate) use crate::a as A;\n#[cfg(all())] pub(crate) use crate::a::Value as V;\n#[cfg(all())] pub(crate) use crate::a::*;\n#[cfg(all())] pub(crate) use crate::b::Other;"
    );

    for source in [
        "use crate::{a::Value, /* keep */ b::Other};",
        "#[observe] use crate::{a::Value, b::Other};",
        "#[expect(unused_imports)] use crate::{a::Value, b::Other};",
        "#[cfg_attr(all(), expect(unused_imports))] use crate::{a::Value, b::Other};",
    ] {
        let root = parse(source);
        let segments = self::segments(source, &["crate", "a", "Value"]);
        assert!(analysis::split_import(&root, source, &segments, &[]).is_none());
    }
}

#[test]
fn split_applies_each_branch_plan_and_preserves_unplanned_routes() {
    for route in [
        "crate::bridge::Value as ViaBridge",
        "::std as Standard",
        "::std::fmt as Format",
        "::std::cmp::*",
        "::std::{fmt::Display as DisplayAlias, io::{self as Io, Read as _}}",
        ":: std :: fmt as Format",
    ] {
        let source = format!("use {{crate::a::{{Value as Parent, b::{{Local as Own, *}}}}, {route}}};");
        let root = parse(&source);
        let imports: Vec<_> = [
            (&["crate", "a", "Value"][..], 1, 2),
            (&["crate", "a", "b", "Local"][..], 2, 3),
            (&["crate", "a", "b"][..], 2, 3),
        ]
        .into_iter()
        .map(|(names, base, consumed)| {
            let segments = segments(&source, names);
            let Some(plan) = analysis::plan(&segments, 2, base, consumed) else {
                panic!("missing branch plan")
            };

            (segments, plan)
        })
        .collect();

        let plans: Vec<_> = imports
            .iter()
            .map(|(segments, plan)| (segments.as_slice(), plan))
            .collect();

        assert_eq!(
            analysis::split_import(&root, &source, &imports[0].0, &plans),
            Some(Edit {
                range: TextRange::new(0.into(), TextSize::of(&source)),
                replacement: format!(
                    "use super::Value as Parent;\nuse self::Local as Own;\nuse self::*;\nuse {route};"
                ),
            }),
            "{route}"
        );
    }
}
