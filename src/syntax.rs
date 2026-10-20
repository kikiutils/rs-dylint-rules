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

pub(crate) fn crosses_conditional(node: &SyntaxNode, until: &SyntaxNode) -> bool {
    node.ancestors()
        .take_while(|ancestor| ancestor != until)
        .any(|ancestor| {
            ancestor
                .children()
                .filter_map(ast::Attr::cast)
                .any(|attr| conditional_attribute(&attr))
        })
}

// These attributes do not inspect the rewritten item tokens.
// Check every cfg_attr branch, including branches inactive in this compilation.
fn inert_meta(meta: ast::Meta) -> bool {
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

pub(crate) fn conditional_attribute(attribute: &ast::Attr) -> bool {
    attribute
        .meta()
        .and_then(|meta| meta.simple_name())
        .is_some_and(|name| matches!(name.as_str(), "cfg" | "cfg_attr"))
}

// Splitting an item must not duplicate lint expectation IDs or their scope.
pub(crate) fn has_expectation(meta: ast::Meta) -> bool {
    if let ast::Meta::CfgAttrMeta(meta) = meta {
        return meta.metas().any(has_expectation);
    }

    meta.simple_name().is_some_and(|name| name == "expect")
}
