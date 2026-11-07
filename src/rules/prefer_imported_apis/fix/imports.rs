use ra_ap_syntax::{
    AstNode,
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
    let offset = scope
        .children()
        .find(|node| !ast::AnyAttr::can_cast(node.kind()))
        .map_or_else(
            || {
                if file {
                    Some(scope.text_range().end())
                } else {
                    Some(scope.last_token()?.text_range().start())
                }
            },
            |node| Some(node.text_range().start()),
        )?;

    let offset = if file && source.starts_with('\u{feff}') {
        offset.max(TextSize::of("\u{feff}"))
    } else {
        offset
    };

    let preamble = TextRange::new(scope.text_range().start(), offset);

    // Inner docs describe this scope. Other comments in the insertion preamble
    // can belong to the following item or to an attribute, so leave them alone.
    if scope.descendants_with_tokens().any(|element| {
        preamble.contains_range(element.text_range())
            && matches!(element.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT)
            && !element
                .ancestors()
                .filter_map(ast::DocComment::cast)
                .any(|doc| doc.kind() == ast::AttrKind::Inner)
    }) {
        return None;
    }

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
