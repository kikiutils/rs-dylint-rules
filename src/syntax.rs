use ra_ap_syntax::{
    AstNode,
    SyntaxKind,
    SyntaxNode,
    TextRange,
    ast,
};

pub(crate) fn has_comments(node: &SyntaxNode, range: TextRange) -> bool {
    node.descendants_with_tokens().any(|element| {
        range.contains_range(element.text_range())
            && matches!(element.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT)
    })
}

// These attributes do not observe rewritten tokens or introduce bindings.
// Check every cfg_attr branch, including branches inactive in this compilation.
pub(crate) fn inert_attribute(attribute: &ast::Attr) -> bool {
    attribute.meta().is_some_and(inert_meta)
}

fn inert_meta(meta: ast::Meta) -> bool {
    if let ast::Meta::CfgAttrMeta(meta) = meta {
        return meta.metas().all(inert_meta);
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
        )
    })
}

pub(crate) fn protected(node: &SyntaxNode) -> bool {
    node.ancestors().any(|node| {
        node.children()
            .filter_map(ast::Attr::cast)
            .any(|attribute| !inert_attribute(&attribute))
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
