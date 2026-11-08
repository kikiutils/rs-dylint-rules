use ra_ap_syntax::{
    SyntaxKind,
    SyntaxNode,
};
use rustc_hir::def::Namespace;
use rustc_span::def_id::DefId;

use super::{
    super::{
        names::NameRequest,
        policy::{
            Api,
            Form,
        },
    },
    Access,
    Import,
};
use crate::syntax;

#[derive(Clone, PartialEq, Eq)]
pub(in super::super) struct Binding {
    pub(in super::super) definition: Option<DefId>,
    unresolved: Option<&'static str>,
    pub(in super::super) name: String,
    pub(in super::super) namespace: Namespace,
    pub(in super::super) external: bool,
}

impl Binding {
    pub(in super::super) fn for_api(api: &Api) -> Option<Self> {
        Some(match &api.form {
            Form::Direct => Self {
                definition: Some(api.definition),
                unresolved: None,
                name: api.name.clone(),
                namespace: api.namespace,
                external: false,
            },
            Form::Qualified { qualifier, .. } => Self {
                definition: Some(qualifier.definition),
                unresolved: None,
                name: qualifier.name.to_owned(),
                namespace: Namespace::TypeNS,
                external: qualifier.external,
            },
            Form::Replace { target: Some(target) } => Self {
                definition: target.definition,
                unresolved: target.definition.is_none().then_some(target.source),
                name: api.name.clone(),
                namespace: api.namespace,
                external: false,
            },
            Form::Replace { target: None } => return None,
        })
    }

    pub(in super::super) fn for_import(import: &Import) -> Option<Self> {
        if let Some(api) = &import.api {
            return Self::for_api(api);
        }

        let qualifier = import.qualifier.as_ref()?;
        Some(Self {
            definition: Some(qualifier.definition),
            unresolved: None,
            name: qualifier.name.to_owned(),
            namespace: Namespace::TypeNS,
            external: qualifier.external,
        })
    }

    pub(in super::super) fn same_name(&self, other: &Self) -> bool {
        self.name == other.name && self.namespace == other.namespace
    }
}

pub(in super::super) fn existing_import<'a>(
    access: &Access,
    imports: &'a [Import],
    binding: &Binding,
) -> Option<&'a Import> {
    let scopes = syntax::visible_scopes(&access.node);
    imports
        .iter()
        .filter(|import| {
            !import.conditional
                && Some(import.definition) == binding.definition
                && import.namespace == binding.namespace
                && import.name == binding.name
                && (import.allowed || import.namespace_import || access.api.is_direct())
                && scopes.contains(&import.scope)
                && import.node.text_range().end() <= access.range.start()
        })
        .min_by_key(|import| scopes.iter().position(|scope| *scope == import.scope))
}

pub(in super::super) fn root_available(
    access: &Access,
    target: &SyntaxNode,
    safe: &dyn Fn(NameRequest<'_>) -> bool,
) -> bool {
    let (name, definition) = match &access.api.form {
        Form::Qualified { qualifier, .. } => (qualifier.source.split("::").next().unwrap_or(""), Some(qualifier.root)),
        Form::Replace { target: Some(import) } => (import.source.split("::").next().unwrap_or(""), import.root),
        _ => return false,
    };

    safe(NameRequest {
        fact: access.fact,
        at: access.range,
        name,
        definition,
        namespace: Namespace::TypeNS,
        target: target.text_range(),
        module_target: target.kind() != SyntaxKind::STMT_LIST,
        reuse: true,
        external: true,
        replacement: None,
    })
}
