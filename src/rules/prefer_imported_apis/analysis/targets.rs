use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextSize,
    ast,
};

// Module boundaries are already safe places for module-scoped imports, even
// when the declaration of that module is conditional.
pub(super) fn target(node: &SyntaxNode) -> Option<SyntaxNode> {
    let ancestors: Vec<_> = node.ancestors().collect();
    let module = ancestors
        .iter()
        .position(|node| matches!(node.kind(), SyntaxKind::ITEM_LIST | SyntaxKind::SOURCE_FILE))?;

    let boundary = ancestors[..module].iter().position(has_cfg);
    match boundary {
        None => Some(ancestors[module].clone()),
        Some(boundary) => ancestors[..=boundary]
            .iter()
            .rev()
            .find(|node| node.kind() == SyntaxKind::STMT_LIST)
            .cloned(),
    }
}

pub(super) fn has_cfg(node: &SyntaxNode) -> bool {
    node.children().filter_map(ast::Attr::cast).any(|attr| {
        attr.meta()
            .and_then(|meta| meta.simple_name())
            .is_some_and(|name| name == "cfg")
    })
}

pub(super) fn insertion(target: &SyntaxNode, source: &str) -> TextSize {
    let mut position = target.text_range().start();
    if target.kind() != SyntaxKind::SOURCE_FILE {
        if let Some(token) = target.first_token() {
            position = token.text_range().end();
        }
    } else if source.starts_with('\u{feff}') {
        position += TextSize::of("\u{feff}");
    }

    for child in target.children_with_tokens() {
        if child.text_range().end() <= position {
            continue;
        }

        if let Some(node) = child.as_node() {
            if node.kind() == SyntaxKind::DOC_COMMENT {
                let text = node.text().to_string();
                if text.starts_with("//!") || text.starts_with("/*!") {
                    position = node.text_range().end();
                    if text.starts_with("//!") {
                        position = next_line(source, position);
                    }

                    continue;
                }
            }

            if ast::Attr::cast(node.clone()).is_some_and(|attr| attr.excl_token().is_some()) {
                position = node.text_range().end();
                continue;
            }

            break;
        }

        let Some(token) = child.as_token() else {
            continue;
        };

        if token.kind() == SyntaxKind::SHEBANG {
            position = next_line(source, token.text_range().end());
        } else if !matches!(token.kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT) {
            break;
        }
    }

    position
}

fn next_line(source: &str, position: TextSize) -> TextSize {
    let offset = usize::from(position);
    if source[..offset].ends_with('\n') {
        return position;
    }

    let remaining = &source[offset..];
    remaining
        .find('\n')
        .map_or(position, |newline| position + TextSize::of(&remaining[..=newline]))
}
