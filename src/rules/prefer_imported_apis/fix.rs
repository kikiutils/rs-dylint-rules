use std::collections::BTreeMap;

use ra_ap_syntax::TextRange;

use super::analysis::{
    Access,
    Decision,
};
use crate::diagnostic::{
    Edit,
    Fix,
    RuleOutput,
};

pub(super) fn plan(accesses: &[Access], decisions: &[Decision], output: &mut RuleOutput) {
    let mut edits = Vec::new();
    let mut imports = BTreeMap::<_, Vec<String>>::new();
    for (access, decision) in accesses.iter().zip(decisions) {
        let (replacement, insertion) = match decision {
            Decision::Canonical { replacement, import } => (replacement, import.as_ref()),
            Decision::FullPath(replacement) => (replacement, None),
            Decision::Skip => continue,
        };

        if replacement == &access.spelling && insertion.is_none() {
            continue;
        }

        output.report(access.diagnostic());
        if replacement != &access.spelling {
            edits.push(Edit {
                range: access.range,
                replacement: replacement.clone(),
            });
        }

        if let Some((position, import)) = insertion {
            let entries = imports.entry(*position).or_default();
            if !entries.contains(import) {
                entries.push(import.clone());
            }
        }
    }

    for (position, entries) in imports {
        edits.push(Edit {
            range: TextRange::empty(position),
            replacement: format!("{}\n", entries.join("\n")),
        });
    }

    if let Some(first) = output.issues.first_mut()
        && !edits.is_empty()
    {
        first.fix = Some(Fix { edits });
    }
}
