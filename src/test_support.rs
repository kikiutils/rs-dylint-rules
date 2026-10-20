use ra_ap_syntax::{
    AstNode,
    Edition,
    SourceFile,
    SyntaxNode,
};

use crate::diagnostic::RuleOutput;

pub(crate) fn parse(source: &str) -> SyntaxNode {
    let parsed = SourceFile::parse(source, Edition::Edition2024);
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    parsed.tree().syntax().clone()
}

pub(crate) fn apply(source: &str, output: &RuleOutput) -> String {
    let mut edits: Vec<_> = output
        .issues
        .iter()
        .filter_map(|issue| issue.fix.as_ref())
        .flat_map(|fix| &fix.edits)
        .collect();

    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
    let mut fixed = source.to_owned();
    for edit in edits {
        fixed.replace_range(std::ops::Range::<usize>::from(edit.range), &edit.replacement);
    }

    fixed
}
