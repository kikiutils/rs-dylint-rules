mod analysis;
mod compiler;

pub(super) const RULE: fn(&mut rustc_lint::LintStore) = self::compiler::register;

#[cfg(test)]
mod tests;
