use ra_ap_syntax::{
    AstNode,
    SyntaxKind as K,
    SyntaxNode,
    TextRange,
    ast,
    ast::HasName,
};
use rustc_hir::def::Namespace;
use rustc_span::def_id::DefId;

use super::super::{
    model::Event,
    policy::{
        Api,
        Form,
    },
};

pub(super) struct Binding {
    pub(super) definition: Option<DefId>,
    pub(super) name: String,
    pub(super) namespace: Namespace,
    pub(super) source: String,
    pub(super) root: Option<DefId>,
    pub(super) external: bool,
}

impl Binding {
    pub(super) fn for_api(api: &Api) -> Option<Self> {
        Some(match &api.form {
            Form::Direct => Self {
                definition: Some(api.definition),
                name: api.name.clone(),
                namespace: api.namespace,
                source: api.source.clone(),
                root: api.root,
                external: false,
            },
            Form::Qualified { qualifier, .. } => Self {
                definition: Some(qualifier.definition),
                name: qualifier.name.to_owned(),
                namespace: Namespace::TypeNS,
                source: qualifier.source.to_owned(),
                root: Some(qualifier.root),
                external: qualifier.external,
            },
            Form::Replace { target: Some(target) } => Self {
                definition: target.definition,
                name: api.name.clone(),
                namespace: api.namespace,
                source: target.source.to_owned(),
                root: target.root,
                external: false,
            },
            Form::Replace { target: None } => return None,
        })
    }

    pub(super) fn declaration(&self) -> String {
        if self.source.rsplit("::").next() == Some(self.name.as_str()) {
            format!("use {};", self.source)
        } else {
            format!("use {} as {};", self.source, self.name)
        }
    }
}

pub(super) struct ExistingImport {
    declaration: TextRange,
    definition: DefId,
    name: String,
    namespace: Namespace,
}

impl ExistingImport {
    pub(super) fn read(root: &SyntaxNode, range: TextRange, event: &Event) -> Option<Self> {
        let (definition, name, namespace) = match event {
            Event::Import { api, name, .. } => (api.definition, name, api.namespace),
            Event::NamespaceImport { qualifier, name, .. } => (qualifier.definition, name, Namespace::TypeNS),
            Event::Use { .. } => return None,
        };

        let declaration = root
            .covering_element(range)
            .ancestors()
            .find(|node| matches!(node.kind(), K::USE | K::EXTERN_CRATE))?
            .text_range();

        Some(Self {
            declaration,
            definition,
            name: name.clone(),
            namespace,
        })
    }
}

pub(super) fn reusable(
    access: &SyntaxNode,
    module: &SyntaxNode,
    binding: &Binding,
    imports: &[ExistingImport],
) -> bool {
    for scope in access.ancestors().filter(crate::syntax::is_scope) {
        if scope.children().any(|item| {
            declares(&item, &binding.name, binding.namespace)
                && !imports.iter().any(|import| {
                    import.declaration == item.text_range()
                        && Some(import.definition) == binding.definition
                        && import.name == binding.name
                        && import.namespace == binding.namespace
                })
        }) {
            return false;
        }

        if scope == *module {
            return true;
        }
    }

    false
}

// Check syntax as well as compiler bindings: inactive cfg branches have no HIR.
// Only definitions immediately in this scope compete with its new import.
pub(super) fn vacant(target: &SyntaxNode, name: &str, namespace: Namespace) -> bool {
    !target.children().any(|item| declares(&item, name, namespace))
}

fn declares(item: &SyntaxNode, name: &str, namespace: Namespace) -> bool {
    if let Some(import) = ast::Use::cast(item.clone()) {
        return import.use_tree().is_some_and(|tree| imported_name(&tree, name));
    }

    if let Some(item) = ast::ExternCrate::cast(item.clone()) {
        return namespace == Namespace::TypeNS
            && item.rename().and_then(|rename| rename.name()).map_or_else(
                || {
                    item.name_ref()
                        .is_some_and(|ident| ident.text().trim_start_matches("r#") == name)
                },
                |ident| ident.text().trim_start_matches("r#") == name,
            );
    }

    let relevant = match item.kind() {
        K::FN | K::CONST | K::STATIC => namespace == Namespace::ValueNS,
        K::MACRO_RULES | K::MACRO_DEF => namespace == Namespace::MacroNS,
        K::STRUCT => namespace != Namespace::MacroNS,
        K::MODULE | K::ENUM | K::UNION | K::TRAIT | K::TYPE_ALIAS => namespace == Namespace::TypeNS,
        _ => false,
    };

    relevant
        && item
            .children()
            .filter_map(ast::Name::cast)
            .any(|ident| ident.text().trim_start_matches("r#") == name)
}

fn imported_name(tree: &ast::UseTree, name: &str) -> bool {
    imported_name_in(tree, name, None)
}

fn imported_name_in(tree: &ast::UseTree, name: &str, parent: Option<String>) -> bool {
    // `self` imports the route accumulated by its parent, not a binding named
    // `self`: std::io::{self} and std::{io::{self}} both bind `io`.
    let leaf = tree
        .path()
        .and_then(|path| {
            path.segments()
                .filter_map(|segment| segment.name_ref())
                .filter(|ident| ident.text() != "self")
                .last()
                .map(|ident| ident.text().trim_start_matches("r#").to_owned())
        })
        .or(parent);

    if let Some(list) = tree.use_tree_list() {
        return list.use_trees().any(|tree| imported_name_in(&tree, name, leaf.clone()));
    }

    if let Some(rename) = tree.rename() {
        return rename
            .name()
            .is_some_and(|ident| ident.text().trim_start_matches("r#") == name);
    }

    tree.star_token().is_none() && leaf.as_deref() == Some(name)
}

// Validate each actual leaf binding, not just the path of its enclosing group.
// `std::{self}` binds the expected root; `std::{ffi as std}` does not.
fn imports_expected_root(tree: &ast::UseTree, name: &str, parent: &[String]) -> bool {
    let mut route = parent.to_vec();
    if let Some(path) = tree.path() {
        let mut segments: Vec<_> = path
            .segments()
            .map(|segment| {
                segment.name_ref().map_or_else(
                    || segment.syntax().text().to_string(),
                    |ident| ident.text().trim_start_matches("r#").to_owned(),
                )
            })
            .collect();

        if segments.last().is_some_and(|segment| segment == "self") {
            segments.pop();
        }

        route.extend(segments);
    }

    if let Some(list) = tree.use_tree_list() {
        return list
            .use_trees()
            .all(|child| imports_expected_root(&child, name, &route));
    }

    let bound = tree.rename().map_or_else(
        || route.last().cloned(),
        |rename| {
            rename
                .name()
                .map(|ident| ident.text().trim_start_matches("r#").to_owned())
        },
    );

    bound.as_deref() != Some(name) || (route.len() == 1 && route[0] == name)
}

pub(super) fn root_unshadowed(
    access: &SyntaxNode,
    target: &SyntaxNode,
    name: &str,
    definition: Option<DefId>,
    imports: &[ExistingImport],
) -> bool {
    for scope in access.ancestors().filter(crate::syntax::is_scope) {
        if !scope.children().all(|item| {
            // Facts distinguish a crate alias from an unrelated macro of the
            // same name. Inactive declarations have no facts and still pass
            // through the conservative CST checks below.
            if matches!(item.kind(), K::USE | K::EXTERN_CRATE) {
                let mut known = false;
                for import in imports
                    .iter()
                    .filter(|import| import.declaration == item.text_range() && import.name == name)
                {
                    known = true;
                    if import.namespace == Namespace::TypeNS {
                        return definition == Some(import.definition);
                    }
                }

                if known {
                    return true;
                }
            }

            // An explicit declaration of the expected extern root does not
            // shadow that root, including the ordinary `extern crate` style.
            if let Some(external) = ast::ExternCrate::cast(item.clone()) {
                let original = external
                    .name_ref()
                    .map(|ident| ident.text().trim_start_matches("r#").to_owned());

                let bound = external
                    .rename()
                    .and_then(|rename| rename.name())
                    .map(|ident| ident.text().trim_start_matches("r#").to_owned())
                    .or_else(|| original.clone());

                return bound.as_deref() != Some(name) || original.as_deref() == Some(name);
            }

            if let Some(import) = ast::Use::cast(item.clone()) {
                if let Some(tree) = import.use_tree()
                    && imported_name(&tree, name)
                {
                    return imports_expected_root(&tree, name, &[]);
                }

                return true;
            }

            // Inspect only declarations directly in the lexical scope.
            !matches!(
                item.kind(),
                K::MODULE | K::STRUCT | K::ENUM | K::UNION | K::TRAIT | K::TYPE_ALIAS
            ) || !item
                .children()
                .filter_map(ast::Name::cast)
                .any(|ident| ident.text().trim_start_matches("r#") == name)
        }) {
            return false;
        }

        if scope == *target {
            return true;
        }
    }

    false
}
