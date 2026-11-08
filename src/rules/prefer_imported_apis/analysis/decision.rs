use ra_ap_syntax::{
    SyntaxKind,
    SyntaxNode,
    TextRange,
    TextSize,
};
use rustc_hir::def::Namespace;

use super::{
    super::{
        model::UseKind,
        names::NameRequest,
    },
    Access,
    Checks,
    declarations::{
        self,
        Binding,
    },
    targets,
};

pub(in super::super) enum Decision {
    Canonical {
        replacement: String,
        import: Option<(TextSize, String)>,
    },
    FullPath(String),
    Skip,
}

fn eligible(access: &Access, checks: &Checks<'_>) -> bool {
    access.kind != UseKind::Trait
        && !(checks.protected)(&access.node)
        && !access.node.ancestors().any(|node| {
            matches!(
                node.kind(),
                SyntaxKind::ATTR | SyntaxKind::MACRO_RULES | SyntaxKind::MACRO_DEF
            )
        })
        && !access
            .node
            .descendants_with_tokens()
            .filter_map(ra_ap_syntax::NodeOrToken::into_token)
            .any(|token| {
                matches!(token.kind(), SyntaxKind::COMMENT | SyntaxKind::DOC_COMMENT)
                    && access.range.intersect(token.text_range()).is_some()
            })
}

fn module(access: &Access) -> Option<SyntaxNode> {
    access
        .node
        .ancestors()
        .find(|node| matches!(node.kind(), SyntaxKind::ITEM_LIST | SyntaxKind::SOURCE_FILE))
}

fn root_available(
    access: &Access,
    binding: &Binding,
    target: &SyntaxNode,
    imports: &[declarations::ExistingImport],
    checks: &Checks<'_>,
) -> bool {
    let root = binding.source.split("::").next().unwrap_or("");
    !root.is_empty()
        && module(access)
            .is_some_and(|module| declarations::root_unshadowed(&access.node, &module, root, binding.root, imports))
        && (checks.name)(NameRequest {
            fact: access.fact,
            at: access.range,
            name: root,
            definition: binding.root,
            namespace: Namespace::TypeNS,
            target: target.text_range(),
            module_target: target.kind() != SyntaxKind::STMT_LIST,
            reuse: true,
            external: true,
            covered: &[],
        })
}

pub(super) fn covered(
    accesses: &[Access],
    imports: &[declarations::ExistingImport],
    checks: &Checks<'_>,
) -> Vec<TextRange> {
    accesses
        .iter()
        .filter(|access| {
            eligible(access, checks)
                && !access.is_canonical()
                && Binding::for_api(&access.api)
                    .zip(module(access))
                    .is_some_and(|(binding, target)| root_available(access, &binding, &target, imports, checks))
        })
        .map(|access| access.range)
        .collect()
}

pub(super) fn decide(
    access: &Access,
    source: &str,
    covered: &[TextRange],
    imports: &[declarations::ExistingImport],
    checks: &Checks<'_>,
) -> Decision {
    if !eligible(access, checks) {
        return Decision::Skip;
    }

    // The compiler already resolved this exact canonical spelling to the API.
    if access.is_canonical() {
        return Decision::Canonical {
            replacement: access.spelling.clone(),
            import: None,
        };
    }

    let Some(binding) = Binding::for_api(&access.api) else {
        return Decision::Skip;
    };

    let Some(module) = module(access) else {
        return Decision::Skip;
    };

    let full_path = access.api.full_path();
    let fallback = || {
        if root_available(access, &binding, &module, imports, checks) {
            full_path.clone().map_or(Decision::Skip, Decision::FullPath)
        } else {
            Decision::Skip
        }
    };

    let request = |target: &SyntaxNode, reuse| NameRequest {
        fact: access.fact,
        at: access.range,
        name: &binding.name,
        definition: binding.definition,
        namespace: binding.namespace,
        target: target.text_range(),
        module_target: target.kind() != SyntaxKind::STMT_LIST,
        reuse,
        external: binding.external,
        covered,
    };

    if declarations::reusable(&access.node, &module, &binding, imports) && (checks.name)(request(&module, true)) {
        return Decision::Canonical {
            replacement: access.api.canonical(),
            import: None,
        };
    }

    let Some(target) = targets::target(&access.node) else {
        return fallback();
    };

    if binding.external
        || !root_available(access, &binding, &target, imports, checks)
        || !declarations::vacant(&target, &binding.name, binding.namespace)
        || !(checks.name)(request(&target, false))
    {
        return fallback();
    }

    Decision::Canonical {
        replacement: access.api.canonical(),
        import: Some((targets::insertion(&target, source), binding.declaration())),
    }
}
