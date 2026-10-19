use crate::diagnostic::Edit;

pub(crate) fn apply_edits<'a>(source: &str, edits: impl Iterator<Item = &'a Edit>) -> String {
    let mut edits: Vec<_> = edits.collect();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));
    let mut fixed = source.to_owned();
    for edit in edits {
        fixed.replace_range(std::ops::Range::<usize>::from(edit.range), &edit.replacement);
    }
    fixed
}
