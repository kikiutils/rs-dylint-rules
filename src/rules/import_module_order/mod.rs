mod analysis;
mod compiler;
mod fix;

pub(super) const RULE: fn(&mut rustc_lint::LintStore) = self::compiler::register;

#[cfg(test)]
mod tests;
