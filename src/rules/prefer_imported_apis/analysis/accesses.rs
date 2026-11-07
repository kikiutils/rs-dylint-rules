use kikiutils::types::fx_collections::FxHashSet;
use ra_ap_syntax::{
    AstNode,
    Edition,
    SyntaxNode,
    TextRange,
    TextSize,
    ast,
};

use super::{
    super::{
        model::UseKind,
        policy::{
            Api,
            Form,
        },
    },
    Import,
    paths,
};
use crate::{
    diagnostic::{
        Diagnostic,
        RuleOutput,
    },
    syntax,
};

pub(in super::super) struct Access {
    pub(in super::super) api: Api,
    pub(in super::super) range: TextRange,
    pub(in super::super) node: SyntaxNode,
    pub(in super::super) scope: SyntaxNode,
    pub(in super::super) kind: UseKind,
    pub(in super::super) spelling: String,
    pub(in super::super) conditional: bool,
}

impl Access {
    pub(in super::super) fn is_canonical(&self) -> bool {
        self.spelling == self.api.canonical()
    }

    pub(in super::super) fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            range: self.range,
            message: format!("use this API as `{}`", self.api.canonical()),
        }
    }
}

pub(in super::super) fn read(
    root: &SyntaxNode,
    range: TextRange,
    api: Api,
    kind: UseKind,
    edition: Edition,
) -> Option<Access> {
    let (node, range, spelling, conditional) = match kind {
        UseKind::Macro => {
            let start = TextRange::new(range.start(), range.start() + TextSize::from(1));
            let call = root
                .covering_element(start)
                .ancestors()
                .filter_map(ast::MacroCall::cast)
                .find(|call| call.syntax().text_range().start() == range.start())?;

            let path = call.path()?;
            let spelling = paths::spelling(&path, path.syntax().text_range());
            (path.syntax().clone(), path.syntax().text_range(), spelling, false)
        },
        UseKind::Trait => {
            let element = root.covering_element(range);
            let node = element.as_node().cloned().or_else(|| element.parent())?;
            (node, range, api.name.clone(), false)
        },
        UseKind::Path => {
            let (node, spelling, conditional) = paths::read(root, range, edition)?;
            (node, range, spelling, conditional)
        },
    };

    let scope = syntax::scope(&node)?;
    let conditional = conditional || syntax::crosses_conditional(&node, &scope);
    Some(Access {
        api,
        range,
        node,
        scope,
        kind,
        spelling,
        conditional,
    })
}

pub(in super::super) fn check_import_order(
    access: &Access,
    imports: &[Import],
    reported: &mut FxHashSet<TextRange>,
    output: &mut RuleOutput,
) {
    let (definition, name) = match &access.api.form {
        Form::Direct => (access.api.definition, access.api.name.as_str()),
        Form::Qualified { qualifier, .. } => (qualifier.definition, qualifier.name),
    };

    let scopes = syntax::visible_scopes(&access.node);
    let matching: Vec<_> = imports
        .iter()
        .filter(|import| import.definition == definition && import.name == name && scopes.contains(&import.scope))
        .collect();

    for import in &matching {
        if !import.allowed && import.namespace_import && reported.insert(import.node.text_range()) {
            output.report(Diagnostic {
                range: import.node.text_range(),
                message: format!("import this namespace as `{name}` without a redundant alias"),
            });
        }
    }

    if !matching.is_empty()
        && matching
            .iter()
            .all(|import| import.node.text_range().end() > access.range.start())
    {
        output.report(Diagnostic {
            range: access.range,
            message: format!("place the import for `{name}` above this use"),
        });
    }
}
