use ra_ap_syntax::{
    AstNode,
    Edition,
    SourceFile,
    TextRange,
    TextSize,
};

use super::analysis::{
    self,
    Segment,
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
    for (base, consumed, prefix) in [(2, 3, "self"), (1, 2, "super")] {
        let Some(plan) = analysis::plan(&segments, 2, base, consumed) else {
            panic!("missing plan")
        };
        assert_eq!(plan.replacement, prefix);
    }
    let source = "use super::super::other::Value;";
    let segments = self::segments(source, &["super", "super", "other", "Value"]);
    assert_eq!(
        analysis::plan(&segments, 2, 0, 2).map(|plan| plan.replacement),
        Some("crate".to_owned())
    );
}

#[test]
fn comments_groups_and_enclosing_attributes_block_unsafe_edits() {
    for (source, names, consumed, allowed) in [
        ("use crate::a::b::Value;", vec!["crate", "a", "b", "Value"], 3, true),
        (
            "use crate /*keep*/ ::a::b::Value;",
            vec!["crate", "a", "b", "Value"],
            3,
            false,
        ),
        ("use crate::{a::b::Value};", vec!["crate", "a", "b", "Value"], 3, false),
        (
            "#[observe] mod m { use crate::a::b::Value; }",
            vec!["crate", "a", "b", "Value"],
            3,
            false,
        ),
        ("use crate::a::b as alias;", vec!["crate", "a", "b"], 3, false),
    ] {
        let parsed = SourceFile::parse(source, Edition::Edition2024);
        let segments = segments(source, &names);
        let Some(plan) = analysis::plan(&segments, 2, 2, consumed) else {
            panic!("missing plan")
        };
        assert_eq!(
            analysis::edit(parsed.tree().syntax(), &segments, &plan, false).is_some(),
            allowed,
            "{source}"
        );
    }
}
