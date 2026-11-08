use ra_ap_syntax::{
    AstNode,
    Direction,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast::{
        self,
        HasVisibility,
    },
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

    // Comments inside a use declaration and movable attributes keep their original
    // text. Other comments and attributes remain ownership or ordering barriers.
    if scope.descendants_with_tokens().any(|element| {
        if range.intersect(element.text_range()).is_none() {
            return false;
        }

        match element.kind() {
            SyntaxKind::COMMENT => !element
                .ancestors()
                .find_map(ast::Use::cast)
                .and_then(|import| declaration_range(&import))
                .is_some_and(|range| range.contains_range(element.text_range())),
            SyntaxKind::ATTR => ast::Attr::cast(element.into_node().unwrap_or_else(|| unreachable!()))
                .and_then(|attribute| attribute.meta())
                .is_none_or(|meta| !movable_meta(meta)),
            SyntaxKind::DOC_COMMENT => true,
            _ => false,
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

// Keep activation and lint scope attached to the declaration. Attributes such
// as macro_use or unknown macros can affect later items, so they remain barriers.
fn movable_meta(meta: ast::Meta) -> bool {
    if let ast::Meta::CfgAttrMeta(cfg) = meta {
        return cfg.metas().all(movable_meta);
    }

    meta.simple_name()
        .is_some_and(|name| matches!(name.as_str(), "cfg" | "allow" | "warn" | "deny" | "forbid"))
}

// Leading comments and attributes belong to the item node but stay outside the moved declaration.
fn declaration_range(import: &ast::Use) -> Option<TextRange> {
    let start = match import.visibility() {
        Some(visibility) => visibility.syntax().text_range().start(),
        None => import.use_token()?.text_range().start(),
    };

    Some(TextRange::new(start, import.semicolon_token()?.text_range().end()))
}
