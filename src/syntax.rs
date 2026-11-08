use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

pub(crate) fn has_comments(node: &SyntaxNode, range: TextRange) -> bool {
    let covering = node.covering_element(range);
    if let Some(token) = covering.as_token() {
        return range.contains_range(token.text_range())
            && matches!(token.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT);
    }

    covering.as_node().is_some_and(|node| {
        node.descendants_with_tokens().any(|element| {
            range.contains_range(element.text_range())
                && matches!(element.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT)
        })
    })
}

pub(crate) fn enclosing<N: AstNode>(root: &SyntaxNode, range: TextRange) -> Option<N> {
    root.covering_element(range).ancestors().find_map(N::cast)
}

pub(crate) fn is_scope(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::STMT_LIST | SyntaxKind::ITEM_LIST | SyntaxKind::SOURCE_FILE
    )
}

// These attributes do not inspect the rewritten item tokens.
// Check every cfg_attr branch, including branches inactive in this compilation.
pub(crate) fn inert_meta(meta: ast::Meta) -> bool {
    if let ast::Meta::CfgAttrMeta(meta) = meta {
        return meta.metas().all(inert_meta);
    }

    if let ast::Meta::UnsafeMeta(meta) = meta {
        return meta.meta().is_some_and(inert_meta);
    }

    // Tool attributes such as `rustfmt::skip` are always multi-segment paths.
    if meta.path().is_some_and(|path| {
        path.qualifier().is_some()
            && path
                .first_segment()
                .and_then(|segment| segment.name_ref())
                .is_some_and(|name| matches!(name.text(), "rustfmt" | "clippy" | "diagnostic"))
    }) {
        return true;
    }

    // tokio: re-emits the function body as raw tokens inside an async block.
    if meta
        .path()
        .is_some_and(|path| matches!(path.to_string().as_str(), "tokio::test" | "tokio::main"))
    {
        return true;
    }

    meta.simple_name().is_some_and(|name| {
        matches!(
            name.as_str(),
            "cfg"
                | "allow"
                | "warn"
                | "deny"
                | "forbid"
                | "expect"
                | "doc"
                | "deprecated"
                | "must_use"
                | "non_exhaustive"
                | "repr"
                | "path"
                | "feature"
                | "no_std"
                | "no_implicit_prelude"
                | "recursion_limit"
                | "type_length_limit"
                | "crate_type"
                | "crate_name"
                | "windows_subsystem"
                | "macro_use"
                | "inline"
                | "cold"
                | "track_caller"
                | "no_mangle"
                | "export_name"
                | "link_section"
                | "link_name"
                | "link"
                | "used"
                | "target_feature"
                | "automatically_derived"
                | "macro_export"
                | "test"
                | "ignore"
                | "should_panic"
                // serde_with: only annotates Option fields and re-emits the item unchanged.
                | "skip_serializing_none"
        )
    })
}

pub(crate) fn protected(node: &SyntaxNode) -> bool {
    protected_with(node, |_| false)
}

pub(crate) fn protected_with(node: &SyntaxNode, extra_safe: impl Fn(&ast::Attr) -> bool) -> bool {
    node.ancestors().any(|node| {
        node.children()
            .filter_map(ast::Attr::cast)
            .any(|attribute| !attribute.meta().is_some_and(inert_meta) && !extra_safe(&attribute))
    })
}

// Splitting an item must not duplicate lint expectation IDs or their scope.
pub(crate) fn has_expectation(meta: ast::Meta) -> bool {
    if let ast::Meta::CfgAttrMeta(meta) = meta {
        return meta.metas().any(has_expectation);
    }

    meta.simple_name().is_some_and(|name| name == "expect")
}

pub(crate) fn derive_meta(attribute: &ast::Attr) -> Option<ast::TokenTreeMeta> {
    derive_token_meta(attribute.meta()?)
}

fn derive_token_meta(meta: ast::Meta) -> Option<ast::TokenTreeMeta> {
    let ast::Meta::TokenTreeMeta(meta) = meta else {
        return None;
    };

    let path = meta
        .path()?
        .syntax()
        .text()
        .to_string()
        .replace(char::is_whitespace, "");

    matches!(
        path.as_str(),
        "derive" | "core::derive" | "std::derive" | "::core::derive" | "::std::derive"
    )
    .then_some(meta)
}

pub(crate) fn derive_list(attribute: &ast::Attr) -> Option<(ast::TokenTree, TextRange, Vec<TextRange>)> {
    derive_list_meta(attribute.meta()?)
}

pub(crate) fn derive_list_meta(meta: ast::Meta) -> Option<(ast::TokenTree, TextRange, Vec<TextRange>)> {
    let meta = derive_token_meta(meta)?;
    let tree = meta.token_tree()?;
    let left = tree.l_paren_token()?;
    let right = tree.r_paren_token()?;
    let inner = TextRange::new(left.text_range().end(), right.text_range().start());
    let mut paths = Vec::new();
    let mut start = None;
    let mut end = None;
    for element in tree
        .syntax()
        .children_with_tokens()
        .filter(|element| inner.contains_range(element.text_range()))
    {
        if element.as_node().is_some() {
            return None;
        }

        match element.kind() {
            SyntaxKind::WHITESPACE | SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT => {},
            SyntaxKind::COMMA => {
                if let (Some(start), Some(end)) = (start.take(), end.take()) {
                    paths.push(TextRange::new(start, end));
                } else {
                    return None;
                }
            },
            _ => {
                start.get_or_insert(element.text_range().start());
                end = Some(element.text_range().end());
            },
        }
    }

    if let (Some(start), Some(end)) = (start, end) {
        paths.push(TextRange::new(start, end));
    }

    Some((tree, inner, paths))
}
