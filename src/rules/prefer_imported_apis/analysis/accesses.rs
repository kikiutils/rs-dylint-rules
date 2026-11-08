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
        model::{
            FactKey,
            UseKind,
        },
        policy::Api,
    },
    paths,
};
use crate::diagnostic::Diagnostic;

pub(in super::super) struct Access {
    pub(in super::super) fact: FactKey,
    pub(in super::super) api: Api,
    pub(in super::super) range: TextRange,
    pub(in super::super) node: SyntaxNode,
    pub(in super::super) kind: UseKind,
    pub(in super::super) spelling: String,
}

impl Access {
    pub(in super::super) fn is_canonical(&self) -> bool {
        !self.api.replaces_type() && self.spelling == self.api.canonical()
    }

    pub(in super::super) fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            range: self.range,
            message: if self.api.replaces_type() {
                "use imported `anyhow::Result` instead of the standard `Result`".to_owned()
            } else {
                format!("use this API as `{}`", self.api.canonical())
            },
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
    let fact = FactKey {
        range,
        definition: api.definition,
    };

    let (node, range, spelling) = match kind {
        UseKind::Macro => {
            let start = TextRange::new(range.start(), range.start() + TextSize::from(1));
            let call = root
                .covering_element(start)
                .ancestors()
                .filter_map(ast::MacroCall::cast)
                .find(|call| call.syntax().text_range().start() == range.start())?;

            let path = call.path()?;
            let spelling = paths::spelling(&path, path.syntax().text_range());
            (path.syntax().clone(), path.syntax().text_range(), spelling)
        },
        UseKind::Trait => {
            let element = root.covering_element(range);
            let node = element.as_node().cloned().or_else(|| element.parent())?;
            (node, range, api.name.clone())
        },
        UseKind::Path => {
            let (node, spelling) = paths::read(root, range, edition)?;
            (node, range, spelling)
        },
    };

    Some(Access {
        fact,
        api,
        range,
        node,
        kind,
        spelling,
    })
}
