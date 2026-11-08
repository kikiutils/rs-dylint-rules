use ra_ap_syntax::{
    AstNode,
    SyntaxNode,
    TextRange,
    ast,
};

use crate::syntax;

// Path replacement preserves the item and attribute ordering. It does not need
// the stricter guarantees required to move items or reorder derive invocations.
pub(in super::super) fn protected(
    node: &SyntaxNode,
    resolve: &impl Fn(TextRange) -> Option<&'static [&'static str]>,
) -> bool {
    syntax::protected_with(node, |attribute| {
        let owner = attribute.syntax().ancestors().find(|node| {
            ast::Struct::can_cast(node.kind()) || ast::Enum::can_cast(node.kind()) || ast::Union::can_cast(node.kind())
        });

        let helpers: Vec<_> = owner
            .iter()
            .flat_map(SyntaxNode::children)
            .filter_map(ast::Attr::cast)
            .filter_map(|attribute| attribute.meta())
            .flat_map(derive_paths)
            .filter_map(resolve)
            .flatten()
            .copied()
            .collect();

        attribute.meta().is_some_and(|meta| safe_meta(meta, &helpers, resolve))
    })
}

fn derive_paths(meta: ast::Meta) -> Vec<TextRange> {
    if let ast::Meta::CfgAttrMeta(cfg) = meta {
        return cfg.metas().flat_map(derive_paths).collect();
    }

    syntax::derive_list_meta(meta).map_or_else(Vec::new, |(_, _, paths)| paths)
}

fn safe_meta(
    meta: ast::Meta,
    helpers: &[&str],
    resolve: &impl Fn(TextRange) -> Option<&'static [&'static str]>,
) -> bool {
    if let ast::Meta::CfgAttrMeta(cfg) = meta {
        return cfg.metas().all(|meta| safe_meta(meta, helpers, resolve));
    }

    if let Some((_, _, paths)) = syntax::derive_list_meta(meta.clone()) {
        return paths.into_iter().all(|range| resolve(range).is_some());
    }

    syntax::inert_meta(meta.clone()) || meta.simple_name().is_some_and(|name| helpers.contains(&name.as_str()))
}

#[cfg(test)]
mod tests {
    use ra_ap_syntax::{
        AstNode,
        ast,
    };

    use crate::test_support::parse;

    fn protected(attributes: &str) -> bool {
        let source = format!("{attributes}\npub struct Value {{ field: JsonValue }}");
        let root = parse(&source);
        let name = root
            .descendants()
            .filter_map(ast::NameRef::cast)
            .find(|name| name.text() == "JsonValue")
            .unwrap_or_else(|| panic!("missing field type"));

        super::protected(name.syntax(), &|range| match &source[range] {
            "serde::Serialize" => Some(&["serde"]),
            "Debug" => Some(&[]),
            _ => None,
        })
    }

    #[test]
    fn cfg_attr_derives_and_their_helpers_allow_path_replacement() {
        for attributes in [
            "#[cfg_attr(all(), derive(serde::Serialize))] #[serde(rename_all = \"camelCase\")]",
            "#[cfg_attr(all(), derive(serde::Serialize), serde(rename_all = \"camelCase\"))]",
            "#[cfg_attr(all(), cfg_attr(all(), derive(serde::Serialize)))] #[cfg_attr(all(), serde(rename_all = \"camelCase\"))]",
            "#[cfg_attr(all(), derive(Debug))]",
        ] {
            assert!(!protected(attributes), "{attributes}");
        }
    }

    #[test]
    fn cfg_attr_keeps_unknown_derives_and_unmatched_helpers_protected() {
        for attributes in [
            "#[cfg_attr(all(), derive(other::Observe))]",
            "#[cfg_attr(all(), derive(Debug), serde(rename_all = \"camelCase\"))]",
            "#[cfg_attr(all(), derive(serde::Serialize), observe)]",
        ] {
            assert!(protected(attributes), "{attributes}");
        }
    }
}
