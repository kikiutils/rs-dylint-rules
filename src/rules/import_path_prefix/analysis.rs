use ra_ap_syntax::{
    AstNode,
    SyntaxNode,
    TextRange,
    ast,
};

use crate::{
    diagnostic::{
        Diagnostic,
        Edit,
        Fix,
        Issue,
        RuleOutput,
    },
    syntax,
};

#[derive(Debug, Clone)]
pub(super) struct Segment {
    pub(super) name: String,
    pub(super) range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Plan {
    pub(super) range: TextRange,
    pub(super) replacement: String,
}

// A compiler-resolved use path and the prefix it should be written with.
pub(super) struct Import {
    pub(super) segments: Vec<Segment>,
    pub(super) plan: Plan,
    pub(super) glob: bool,
}

pub(super) fn analyze(root: &SyntaxNode, source: &str, imports: &[&Import]) -> RuleOutput {
    let mut output = RuleOutput::default();
    let mut emitted = Vec::new();
    let mut split_ranges = Vec::new();
    let plans: Vec<_> = imports
        .iter()
        .map(|import| (import.segments.as_slice(), &import.plan))
        .collect();

    for import in imports {
        // Macro inputs are token trees, not source use items. Expansion
        // tokens can retain original spans even without expansion ctxt.
        if syntax::enclosing::<ast::Use>(root, import.segments[0].range).is_none() {
            continue;
        }

        if emitted.contains(&import.plan) {
            continue;
        }

        emitted.push(import.plan.clone());
        let edit = edit(root, source, &import.segments, &import.plan, import.glob).filter(|edit| {
            imports
                .iter()
                .all(|other| other.plan == import.plan || other.plan.range.intersect(edit.range).is_none())
        });

        let edit = edit.or_else(|| {
            let split = split_import(root, source, &import.segments, &plans)?;
            if split_ranges.contains(&split.range) {
                return None;
            }

            split_ranges.push(split.range);
            Some(split)
        });

        output.issues.push(Issue {
            diagnostics: vec![Diagnostic {
                range: import.plan.range,
                message: format!("expected import prefix `{}`", import.plan.replacement),
            }],
            fix: edit.map(|edit| Fix { edits: vec![edit] }),
        });
    }

    output
}

// The compiler has already established the starting module and the direct
// ancestry segments. Never derive ancestry from source-file names.
pub(super) fn plan(segments: &[Segment], depth: usize, base: usize, consumed: usize) -> Option<Plan> {
    let first = segments.first()?;
    let prefix = if base == depth {
        "self".to_owned()
    } else if base == 0 {
        "crate".to_owned()
    } else {
        std::iter::repeat_n("super", depth - base)
            .collect::<Vec<_>>()
            .join("::")
    };

    if consumed == 0 {
        return Some(Plan {
            range: TextRange::empty(first.range.start()),
            replacement: format!("{prefix}::"),
        });
    }

    let last = &segments[consumed - 1];
    let actual = segments[..consumed]
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::");

    (actual != prefix).then_some(Plan {
        range: TextRange::new(first.range.start(), last.range.end()),
        replacement: prefix,
    })
}

pub(super) fn edit(root: &SyntaxNode, source: &str, segments: &[Segment], plan: &Plan, glob: bool) -> Option<Edit> {
    // Importing the anchor module itself needs a grouped self import. Report it
    // without inventing a new use-tree shape.
    if !glob && segments.last()?.range.end() == plan.range.end() {
        return None;
    }

    let import = syntax::enclosing::<ast::Use>(root, segments[0].range)?;

    if syntax::protected(import.syntax()) {
        return None;
    }

    // Replacements may span multiple segments only when they are a contiguous
    // path. In particular, never erase braces, aliases, comments or siblings.
    for pair in segments.windows(2) {
        let gap = TextRange::new(pair[0].range.end(), pair[1].range.start());
        if !plan.range.contains_range(gap) {
            continue;
        }

        if source[gap]
            .chars()
            .filter(|character| !character.is_whitespace())
            .ne("::".chars())
        {
            return None;
        }
    }

    Some(Edit {
        range: plan.range,
        replacement: plan.replacement.clone(),
    })
}

// Rebuild comment-free branches independently, applying compiler-confirmed
// prefix plans while preserving all other import routes.
pub(super) fn split_import(
    root: &SyntaxNode,
    source: &str,
    segments: &[Segment],
    plans: &[(&[Segment], &Plan)],
) -> Option<Edit> {
    let first = segments.first()?;
    let import = syntax::enclosing::<ast::Use>(root, first.range)?;

    if syntax::protected(import.syntax())
        || syntax::has_comments(import.syntax(), import.syntax().text_range())
        || import
            .syntax()
            .children()
            .filter_map(ast::Attr::cast)
            .filter_map(|attr| attr.meta())
            .any(syntax::has_expectation)
    {
        return None;
    }

    let tree = import.use_tree()?;
    let mut paths = Vec::new();
    flatten(&tree, &[], plans, &mut paths)?;
    if paths.len() < 2 && tree.use_tree_list().is_none() {
        return None;
    }

    let prefix = &source[TextRange::new(import.syntax().text_range().start(), tree.syntax().text_range().start())];

    let replacement = paths
        .iter()
        .map(|path| format!("{prefix}{path};"))
        .collect::<Vec<_>>()
        .join("\n");

    (!paths.is_empty()).then_some(Edit {
        range: import.syntax().text_range(),
        replacement,
    })
}

fn flatten(
    tree: &ast::UseTree,
    prefix: &[Segment],
    plans: &[(&[Segment], &Plan)],
    output: &mut Vec<String>,
) -> Option<()> {
    let mut segments = prefix.to_vec();
    if let Some(path) = tree.path() {
        // Absolute extern-prelude paths are not rewritten by this rule.
        if path.syntax().text().to_string().trim_start().starts_with("::") {
            return None;
        }

        segments.extend(path.segments().map(|segment| Segment {
            name: segment.syntax().text().to_string(),
            range: segment.syntax().text_range(),
        }));
    }

    if let Some(list) = tree.use_tree_list() {
        for child in list.use_trees() {
            flatten(&child, &segments, plans, output)?;
        }

        return Some(());
    }

    let mut names: Vec<_> = segments.iter().map(|segment| segment.name.clone()).collect();
    if let Some((_, plan)) = plans.iter().find(|(path, _)| {
        path.iter()
            .map(|segment| segment.range)
            .eq(segments.iter().map(|segment| segment.range))
    }) {
        let consumed = if plan.range.is_empty() {
            0
        } else {
            segments
                .iter()
                .position(|segment| segment.range.end() == plan.range.end())?
                + 1
        };

        // Anchor-module imports still require their own grouped-self policy.
        if consumed < names.len() || tree.star_token().is_some() {
            names.splice(..consumed, [plan.replacement.trim_end_matches("::").to_owned()]);
        }
    }

    if names.len() > 1 && names.last().is_some_and(|name| name == "self") {
        names.pop();
    }

    let mut full = names.join("::");
    if tree.star_token().is_some() {
        if !full.is_empty() {
            full.push_str("::");
        }

        full.push('*');
    }

    if let Some(rename) = tree.rename() {
        full.push(' ');
        full.push_str(&rename.syntax().text().to_string());
    }

    if full.is_empty() {
        return None;
    }

    output.push(full);
    Some(())
}
