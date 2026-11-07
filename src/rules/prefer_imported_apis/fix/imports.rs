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

pub(super) struct Insertion<'a> {
    offset: TextSize,
    newline: &'static str,
    indent: &'a str,
    leading_newline: bool,
}

impl Insertion<'_> {
    pub(super) fn edit<'a>(&self, statements: impl IntoIterator<Item = &'a str>) -> Edit {
        let mut replacement = String::new();
        if self.leading_newline {
            replacement.push_str(self.newline);
        }

        for (index, statement) in statements.into_iter().enumerate() {
            if index > 0 {
                replacement.push_str(self.indent);
            }

            replacement.push_str(statement);
            replacement.push_str(self.newline);
        }

        replacement.push_str(self.indent);

        Edit {
            range: TextRange::empty(self.offset),
            replacement,
        }
    }
}

pub(super) fn insertion<'a>(scope: &SyntaxNode, source: &'a str) -> Option<Insertion<'a>> {
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

    let index = usize::from(offset);
    let newline = if source.contains("\r\n") { "\r\n" } else { "\n" };
    let line_start = source[..index].rfind('\n').map_or(0, |position| position + 1);
    let prefix = &source[line_start..index];
    let indent = if prefix.chars().all(|character| matches!(character, ' ' | '\t')) {
        prefix
    } else {
        ""
    };

    Some(Insertion {
        offset,
        newline,
        indent,
        leading_newline: !file && !prefix.chars().all(|character| matches!(character, ' ' | '\t')),
    })
}

#[cfg(test)]
#[path = "tests/imports.rs"]
mod tests;
