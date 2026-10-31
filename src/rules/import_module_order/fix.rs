use ra_ap_syntax::{
    AstNode,
    Direction,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

use super::analysis::Entry;
use crate::{
    diagnostic::Edit,
    syntax,
};

pub(super) fn reorder(scope: &SyntaxNode, entries: &[Entry]) -> Option<Edit> {
    if syntax::protected(scope) {
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

    // Comments inside a use tree move with its unchanged text. Attributes and
    // other comments remain barriers because their ownership may be unclear.
    if scope.descendants_with_tokens().any(|element| {
        if range.intersect(element.text_range()).is_none() {
            return false;
        }

        match element.kind() {
            SyntaxKind::COMMENT => !element.ancestors().any(|node| ast::UseTree::can_cast(node.kind())),
            kind => ast::AnyAttr::can_cast(kind),
        }
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

    let mut ordered: Vec<_> = entries.iter().collect();
    ordered.sort_by_key(|entry| entry.group);
    let mut replacement = String::new();
    let mut previous = None;

    for entry in ordered {
        if let Some(group) = previous {
            replacement.push('\n');

            if group != entry.group {
                replacement.push('\n');
            }
        }

        replacement.push_str(&entry.node.text().to_string());
        previous = Some(entry.group);
    }

    Some(Edit { range, replacement })
}
