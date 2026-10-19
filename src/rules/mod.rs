mod import_module_order;

pub(crate) const RULES: &[fn(&mut rustc_lint::LintStore)] = &[self::import_module_order::RULE];
