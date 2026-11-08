use ra_ap_syntax::{
    AstNode,
    NodeOrToken,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    TextSize,
    ast,
};

use crate::{
    diagnostic::Edit,
    syntax,
};

pub(super) struct Insertion {
    offset: TextSize,
}

impl Insertion {
    pub(super) fn edit<'a>(&self, statements: impl IntoIterator<Item = &'a str>) -> Edit {
        let replacement = statements.into_iter().flat_map(|statement| [statement, "\n"]).collect();
        Edit {
            range: TextRange::empty(self.offset),
            replacement,
        }
    }
}

pub(super) fn insertion(scope: &SyntaxNode, source: &str) -> Option<Insertion> {
    if !syntax::is_scope(scope) || syntax::protected(scope) {
        return None;
    }

    let file = scope.kind() == SyntaxKind::SOURCE_FILE;
    let attributes_end = scope
        .children()
        .take_while(|node| ast::AnyAttr::can_cast(node.kind()))
        .last()
        .map_or(scope.text_range().start(), |node| node.text_range().end());

    let first = scope.children().find(|node| !ast::AnyAttr::can_cast(node.kind()));

    let offset = first.map_or_else(
        || {
            if file {
                Some(scope.text_range().end())
            } else {
                Some(scope.last_token()?.text_range().start())
            }
        },
        |node| {
            node.descendants_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .find(|token| {
                    !matches!(
                        token.kind(),
                        SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT
                    )
                })
                .map(|token| token.text_range().start())
        },
    )?;

    // CST comment attachment differs for items, statements and CRLF input.
    // Keep detached banners in place, but insert before attached comments/docs.
    let mut attached = None;
    let mut outer_docs = false;
    for token in scope
        .descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .take_while(|token| token.text_range().end() <= offset)
        .filter(|token| token.text_range().start() >= attributes_end)
    {
        match token.kind() {
            SyntaxKind::WHITESPACE => {
                if !outer_docs && token.text().matches('\n').count() > 1 {
                    attached = None;
                }
            },
            SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT => {
                // A same-line comment after an inner attribute stays with it.
                if token.kind() == SyntaxKind::COMMENT
                    && attributes_end > scope.text_range().start()
                    && !source[TextRange::new(attributes_end, token.text_range().start())].contains('\n')
                {
                    continue;
                }

                attached.get_or_insert(token.text_range().start());
                outer_docs |= token.kind() == SyntaxKind::DOC_COMMENT;
            },
            _ => attached = None,
        }
    }

    let offset = attached.unwrap_or(offset);
    let offset = if file && source.starts_with('\u{feff}') {
        offset.max(TextSize::of("\u{feff}"))
    } else {
        offset
    };

    Some(Insertion { offset })
}

#[cfg(test)]
#[path = "tests/imports.rs"]
mod tests;
