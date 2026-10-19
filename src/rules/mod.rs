mod grouped_derive_order;
mod import_module_order;
mod import_path_prefix;

pub(crate) const RULES: &[fn(&mut rustc_lint::LintStore)] = &[
    self::import_module_order::RULE,
    self::import_path_prefix::RULE,
    self::grouped_derive_order::RULE,
];
