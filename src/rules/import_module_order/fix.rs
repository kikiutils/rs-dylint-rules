use ra_ap_syntax::{
    AstNode,
    Direction,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

use super::analysis::Entry;
use crate::diagnostic::Edit;

// Functions
pub(super) fn reorder(scope: &SyntaxNode, entries: &[Entry]) -> Option<Edit> {
    // An enclosing attribute macro can observe the entire item's token order.
    // Conservatively treat all enclosing attributes, including inner attributes
    // and cfg_attr, as barriers until their safety can be established.
    if scope
        .ancestors()
        .any(|ancestor| ancestor.children().any(|node| ast::Attr::can_cast(node.kind())))
    {
        return None;
    }

    let first = entries.first()?;
    let last = entries.last()?;
    let range = TextRange::new(first.node.text_range().start(), last.node.text_range().end());

    // Never cross executable statements, inline modules, macros or other items.
    if scope
        .children()
        .filter(|node| range.contains_range(node.text_range()))
        .count()
        != entries.len()
    {
        return None;
    }

    // Attributes can run macros or affect module loading. Standalone comments
    // have no reliable item ownership, so these cases remain diagnostic-only.
    if scope.descendants_with_tokens().any(|element| {
        let relevant = range.intersect(element.text_range()).is_some();
        relevant
            && (matches!(
                element.kind(),
                SyntaxKind::ATTR | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT
            ) || element
                .as_node()
                .is_some_and(|node| ast::AnyAttr::can_cast(node.kind())))
    }) {
        return None;
    }

    // A comment immediately before/after the region may belong to its edge item.
    for entry in [first, last] {
        for direction in [Direction::Prev, Direction::Next] {
            for next in entry.node.siblings_with_tokens(direction).skip(1) {
                if next.kind() == SyntaxKind::WHITESPACE {
                    if next.to_string().matches('\n').count() > 1 {
                        break;
                    }
                } else {
                    if matches!(next.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT) {
                        return None;
                    }

                    break;
                }
            }
        }
    }

    let source = scope.text().to_string();
    let relative = usize::from(range.start() - scope.text_range().start());
    let line_start = source[..relative].rfind('\n').map_or(0, |offset| offset + 1);
    let indent = &source[line_start..relative];

    // Same-line declarations need a formatter rather than guessed indentation.
    if !indent.chars().all(|character| matches!(character, ' ' | '\t')) {
        return None;
    }

    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let mut ordered: Vec<_> = entries.iter().collect();
    ordered.sort_by_key(|entry| entry.group);
    let mut replacement = String::new();
    let mut previous = None;

    for entry in ordered {
        if let Some(group) = previous {
            replacement.push_str(newline);

            if group != entry.group {
                replacement.push_str(newline);
            }

            replacement.push_str(indent);
        }

        replacement.push_str(&entry.node.text().to_string());
        previous = Some(entry.group);
    }

    Some(Edit { range, replacement })
}
