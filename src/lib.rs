#![feature(rustc_private)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_lint_defs;
extern crate rustc_session;
extern crate rustc_span;

mod diagnostic;
mod facts;
mod pass;
mod rules;
mod source;
mod syntax;
#[cfg(test)]
mod test_support;
mod workspace;

// Match the compiler ABI; Dylint supplies and owns the actual driver.
dylint_linting::dylint_library!();

// Dylint loads this symbol using the compiler's Rust ABI.
#[unsafe(no_mangle)]
#[allow(clippy::no_mangle_with_rust_abi)]
pub fn register_lints(_session: &rustc_session::Session, store: &mut rustc_lint::LintStore) {
    store.register_lints(&rules::lints());
    store.register_late_lint_pass(Box::new(|_| Box::<pass::KikiRules>::default()));
}
