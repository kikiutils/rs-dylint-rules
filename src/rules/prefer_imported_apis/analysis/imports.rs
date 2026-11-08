use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::{
    AstNode,
    SyntaxNode,
    TextRange,
    ast,
};
use rustc_hir::def::Namespace;
use rustc_span::def_id::DefId;

use super::super::{
    model::Event,
    policy::{
        Api,
        Form,
        Qualifier,
    },
};
use crate::{
    diagnostic::{
        Diagnostic,
        RuleOutput,
    },
    syntax,
};

pub(in super::super) struct Import {
    pub(in super::super) origin: rustc_hir::HirId,
    pub(in super::super) range: TextRange,
    pub(in super::super) api: Option<Api>,
    pub(in super::super) qualifier: Option<Qualifier>,
    pub(in super::super) definition: DefId,
    pub(in super::super) namespace: Namespace,
    pub(in super::super) name: String,
    pub(in super::super) node: SyntaxNode,
    pub(in super::super) scope: SyntaxNode,
    pub(in super::super) conditional: bool,
    pub(in super::super) allowed: bool,
    pub(in super::super) namespace_import: bool,
}

pub(in super::super) fn read(
    root: &SyntaxNode,
    range: TextRange,
    event: &Event,
    origin: rustc_hir::HirId,
    reported: &mut FxHashSet<TextRange>,
    output: &mut RuleOutput,
) -> Option<Import> {
    let tree = syntax::enclosing::<ast::UseTree>(root, range);
    let external = syntax::enclosing::<ast::ExternCrate>(root, range);
    let node = tree
        .as_ref()
        .map(|tree| tree.syntax().clone())
        .or_else(|| external.as_ref().map(|item| item.syntax().clone()))?;

    let scope = syntax::scope(&node)?;
    let conditional = node
        .ancestors()
        .filter(|node| {
            matches!(
                node.kind(),
                ra_ap_syntax::SyntaxKind::USE | ra_ap_syntax::SyntaxKind::EXTERN_CRATE
            )
        })
        .any(|item| syntax::has_conditional_attribute(&item));

    let (definition, namespace, name, allowed, namespace_import) = match event {
        Event::Import { api, name, renamed } => {
            let allowed = matches!(api.form, Form::Direct) && name == &api.name && !renamed;
            if !allowed && reported.insert(range) {
                output.report(Diagnostic {
                    range,
                    message: if api.replaces_type() {
                        "import `anyhow::Result` instead of the standard `Result`".to_owned()
                    } else {
                        format!(
                            "use `{}` instead of directly importing this API with an unsupported name",
                            api.canonical()
                        )
                    },
                });
            }

            (api.definition, api.namespace, name.clone(), allowed, false)
        },
        Event::NamespaceImport {
            qualifier,
            name,
            renamed,
        } => {
            let leaf = tree
                .as_ref()
                .and_then(ast::UseTree::path)
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref())
                .or_else(|| external.as_ref().and_then(ast::ExternCrate::name_ref))
                .map(|name| name.text().trim_start_matches("r#").to_owned());

            let redundant = *renamed && leaf.as_deref() == Some(name.as_str());
            (
                qualifier.definition,
                Namespace::TypeNS,
                name.clone(),
                name == qualifier.name && !redundant,
                true,
            )
        },
        Event::Use { .. } => return None,
    };

    Some(Import {
        origin,
        range,
        api: match event {
            Event::Import { api, .. } => Some(api.clone()),
            _ => None,
        },
        qualifier: match event {
            Event::NamespaceImport { qualifier, .. } => Some(qualifier.clone()),
            _ => None,
        },
        definition,
        namespace,
        name,
        node,
        scope,
        conditional,
        allowed,
        namespace_import,
    })
}
