use ra_ap_syntax::{
    AstNode,
    SyntaxNode,
    TextRange,
    ast,
};

use crate::diagnostic::Edit;

#[derive(Debug, Clone)]
pub(super) struct Segment {
    pub(super) name: String,
    pub(super) range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Plan {
    pub(super) range: TextRange,
    pub(super) replacement: String,
}

// The compiler has already established the starting module and the direct
// ancestry segments. Never derive ancestry from source-file names.
pub(super) fn plan(segments: &[Segment], depth: usize, base: usize, consumed: usize) -> Option<Plan> {
    let first = segments.first()?;
    let prefix = if base == depth {
        "self".to_owned()
    } else if base == 0 {
        "crate".to_owned()
    } else {
        std::iter::repeat_n("super", depth - base)
            .collect::<Vec<_>>()
            .join("::")
    };
    if consumed == 0 {
        return Some(Plan {
            range: TextRange::empty(first.range.start()),
            replacement: format!("{prefix}::"),
        });
    }
    let last = &segments[consumed - 1];
    let actual = segments[..consumed]
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::");
    (actual != prefix).then_some(Plan {
        range: TextRange::new(first.range.start(), last.range.end()),
        replacement: prefix,
    })
}

pub(super) fn edit(root: &SyntaxNode, segments: &[Segment], plan: &Plan, glob: bool) -> Option<Edit> {
    // Importing the anchor module itself needs a grouped self import. Report it
    // without inventing a new use-tree shape.
    if !glob && segments.last()?.range.end() == plan.range.end() {
        return None;
    }
    let import = root
        .descendants()
        .filter_map(ast::Use::cast)
        .find(|item| item.syntax().text_range().contains_range(segments[0].range))?;
    if import
        .syntax()
        .ancestors()
        .any(|node| node.children().any(|child| ast::Attr::can_cast(child.kind())))
    {
        return None;
    }
    // Replacements may span multiple segments only when they are a contiguous
    // path. In particular, never erase braces, aliases, comments or siblings.
    let source = root.text().to_string();
    let base = root.text_range().start();
    for pair in segments.windows(2) {
        let gap = TextRange::new(pair[0].range.end(), pair[1].range.start());
        if !plan.range.contains_range(gap) {
            continue;
        }
        let text = &source[std::ops::Range::<usize>::from(gap - base)];
        if text
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
            != "::"
        {
            return None;
        }
    }
    Some(Edit {
        range: plan.range,
        replacement: plan.replacement.clone(),
    })
}
