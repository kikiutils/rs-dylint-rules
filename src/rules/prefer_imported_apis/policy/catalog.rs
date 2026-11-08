use kikiutils::types::fx_collections::FxHashMap;
use rustc_hir::def::{
    DefKind,
    Res,
};
use rustc_lint::{
    LateContext,
    LintContext,
};
use rustc_span::{
    def_id::{
        CRATE_DEF_INDEX,
        DefId,
    },
    sym,
};

use super::{
    Api,
    Form,
    ImportTarget,
    Qualifier,
    families::{
        self,
        Family,
        Members,
    },
};

#[derive(Default)]
pub(in super::super) struct Catalog {
    apis: FxHashMap<DefId, Selection>,
    qualifiers: FxHashMap<DefId, Qualifier>,
}

struct Selection {
    priority: u8,
    api: Option<Api>,
}

impl Catalog {
    pub(in super::super) fn build(cx: &LateContext<'_>) -> Self {
        let mut catalog = Self::default();
        for family in families::all() {
            let Some(crate_name) = family.module.split("::").next() else {
                continue;
            };

            for &krate in cx.tcx.crates(()) {
                if cx.tcx.crate_name(krate).as_str() != crate_name {
                    continue;
                }

                let root = DefId {
                    krate,
                    index: CRATE_DEF_INDEX,
                };

                catalog.family(cx, root, family);
            }
        }

        for api in catalog.apis.values().filter_map(|selection| selection.api.as_ref()) {
            if let Form::Qualified { qualifier, .. } = &api.form {
                catalog.qualifiers.insert(qualifier.definition, qualifier.clone());
            }
        }

        catalog.standard_result(cx);

        catalog
    }

    pub(in super::super) fn api(&self, definition: DefId) -> Option<&Api> {
        self.apis.get(&definition)?.api.as_ref()
    }

    pub(in super::super) fn qualifier(&self, definition: DefId) -> Option<&Qualifier> {
        self.qualifiers.get(&definition)
    }

    fn standard_result(&mut self, cx: &LateContext<'_>) {
        let Some(definition) = cx.tcx.get_diagnostic_item(sym::Result) else {
            return;
        };

        let targets: Vec<_> = self
            .apis
            .values()
            .filter_map(|selection| selection.api.as_ref())
            .filter_map(|api| {
                let root = DefId {
                    krate: api.definition.krate,
                    index: CRATE_DEF_INDEX,
                };

                (api.name == "Result"
                    && matches!(api.form, Form::Direct)
                    && cx.tcx.crate_name(root.krate).as_str() == "anyhow")
                    .then_some(ImportTarget {
                        source: "anyhow::Result",
                        definition: Some(api.definition),
                        root: Some(root),
                    })
            })
            .collect();

        let target = targets
            .iter()
            .find(|target| {
                target
                    .root
                    .is_some_and(|root| super::super::names::external_crate(cx, "anyhow", root))
            })
            .cloned()
            .or_else(|| (targets.len() == 1).then(|| targets[0].clone()))
            .or_else(|| {
                cx.sess()
                    .opts
                    .externs
                    .get("anyhow")
                    .filter(|entry| entry.add_prelude)
                    .map(|_| ImportTarget {
                        source: "anyhow::Result",
                        definition: None,
                        root: None,
                    })
            });

        self.select(
            Api {
                definition,
                name: "Result".to_owned(),
                source: "anyhow::Result".to_owned(),
                root: target.as_ref().and_then(|target| target.root),
                namespace: rustc_hir::def::Namespace::TypeNS,
                form: Form::Replace { target },
            },
            0,
        );
    }

    fn family(&mut self, cx: &LateContext<'_>, root: DefId, family: &Family) {
        let Some(module) = resolve(cx, root, family.module) else {
            return;
        };

        let qualifier = match family.qualifier {
            Some((source, name)) => {
                let Some(definition) = resolve(cx, root, source) else {
                    return;
                };

                let qualifier = Qualifier {
                    source,
                    name,
                    definition,
                    root,
                    external: !source.contains("::"),
                };

                Some(qualifier)
            },
            None => None,
        };

        self.members(cx, root, module, family, qualifier.as_ref(), "");
    }

    fn members(
        &mut self,
        cx: &LateContext<'_>,
        root: DefId,
        module: DefId,
        family: &Family,
        qualifier: Option<&Qualifier>,
        nested: &str,
    ) {
        for child in cx.tcx.module_children(module) {
            if !child.vis.is_public() {
                continue;
            }

            let Res::Def(kind, definition) = child.res else {
                continue;
            };

            let name = child.ident.name.as_str();
            if matches!(family.members, Members::Channel) && kind == DefKind::Mod && name == "error" {
                self.members(cx, root, definition, family, qualifier, "error::");
                continue;
            }

            let selected = match family.members {
                Members::Functions(excluded) => kind == DefKind::Fn && !excluded.contains(&name),
                Members::Named(names) => names.contains(&name),
                Members::Channel => matches!(
                    kind,
                    DefKind::Fn | DefKind::Struct | DefKind::Enum | DefKind::Union | DefKind::TyAlias
                ),
                Members::Owned { constants } => {
                    definition.krate == root.krate
                        && (matches!(
                            kind,
                            DefKind::Fn | DefKind::Struct | DefKind::Enum | DefKind::Union | DefKind::TyAlias
                        ) || (constants && kind == DefKind::Const))
                },
            };

            if !selected {
                continue;
            }

            let Some(namespace) = child.res.ns() else { continue };
            let form = qualifier.map_or(Form::Direct, |qualifier| {
                let relative = family
                    .module
                    .strip_prefix(qualifier.source)
                    .unwrap_or("")
                    .trim_start_matches("::");

                let member = if relative.is_empty() {
                    format!("{nested}{name}")
                } else {
                    format!("{relative}::{nested}{name}")
                };

                Form::Qualified {
                    qualifier: qualifier.clone(),
                    member,
                }
            });

            let api = Api {
                definition,
                name: name.to_owned(),
                source: format!("{}::{nested}{name}", family.module),
                root: Some(root),
                namespace,
                form,
            };

            self.select(api, family.priority);
        }
    }

    fn select(&mut self, api: Api, priority: u8) {
        self.apis
            .entry(api.definition)
            .and_modify(|selected| {
                if priority < selected.priority {
                    selected.priority = priority;
                    selected.api = Some(api.clone());
                } else if priority == selected.priority && selected.api.as_ref() != Some(&api) {
                    // Ambiguous policies must never depend on family iteration order.
                    selected.api = None;
                }
            })
            .or_insert(Selection {
                priority,
                api: Some(api),
            });
    }
}

fn resolve(cx: &LateContext<'_>, root: DefId, path: &str) -> Option<DefId> {
    let mut definition = root;
    for segment in path.split("::").skip(1) {
        definition = cx.tcx.module_children(definition).iter().find_map(|child| {
            if !child.vis.is_public() || child.ident.name.as_str() != segment {
                return None;
            }

            match child.res {
                Res::Def(DefKind::Mod, definition) => Some(definition),
                _ => None,
            }
        })?;
    }

    Some(definition)
}

#[cfg(test)]
mod tests {
    use rustc_hir::def::Namespace;
    use rustc_span::def_id::LOCAL_CRATE;

    use super::*;

    fn api(source: &'static str) -> Api {
        let definition = DefId {
            krate: LOCAL_CRATE,
            index: CRATE_DEF_INDEX,
        };

        Api {
            definition,
            name: "take".to_owned(),
            source: format!("{source}::take"),
            root: Some(definition),
            namespace: Namespace::ValueNS,
            form: Form::Qualified {
                qualifier: Qualifier {
                    source,
                    name: "mem",
                    definition,
                    root: definition,
                    external: false,
                },
                member: "take".to_owned(),
            },
        }
    }

    #[test]
    fn canonical_routes_use_explicit_priority_not_insertion_order() {
        for reversed in [false, true] {
            let mut catalog = Catalog::default();
            let mut entries = vec![(api("std::mem"), 0), (api("core::mem"), 1)];
            if reversed {
                entries.reverse();
            }

            for (api, priority) in entries {
                catalog.select(api, priority);
            }

            assert_eq!(
                catalog
                    .api(api("std::mem").definition)
                    .and_then(Api::full_path)
                    .as_deref(),
                Some("std::mem::take")
            );
        }
    }

    #[test]
    fn conflicting_policies_are_not_resolved_by_insertion_order() {
        let mut catalog = Catalog::default();
        let first = api("one::mem");
        catalog.select(first.clone(), 0);
        catalog.select(api("two::mem"), 0);
        assert!(catalog.api(first.definition).is_none());
    }
}
