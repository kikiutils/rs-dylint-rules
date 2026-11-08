use ra_ap_syntax::SyntaxKind;

use super::{
    super::{
        names::NameRequest,
        policy::Form,
    },
    Access,
    Import,
    bindings::{
        Binding,
        existing_import,
        root_available,
    },
};

pub(super) fn is_fallback(access: &Access, imports: &[Import], safe: &dyn Fn(NameRequest<'_>) -> bool) -> bool {
    let Form::Qualified { qualifier, .. } = &access.api.form else {
        return false;
    };

    if qualifier.external || access.spelling != access.api.full_path().unwrap_or_default() {
        return false;
    }

    let Some(binding) = Binding::for_api(&access.api) else {
        return false;
    };

    let imported = existing_import(access, imports, &binding);

    let target = imported.map_or(&access.scope, |import| &import.scope);
    let module_target = target.kind() != SyntaxKind::STMT_LIST;
    !safe(NameRequest {
        fact: access.fact,
        at: access.range,
        name: &binding.name,
        definition: binding.definition,
        namespace: binding.namespace,
        target: target.text_range(),
        module_target,
        reuse: imported.is_some(),
        external: binding.external,
        replacement: None,
    }) && root_available(access, target, safe)
}
