#![feature(rustc_private)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_lint_defs;
extern crate rustc_session;
extern crate rustc_span;

mod diagnostic;
mod rules;
mod source;
mod syntax;
#[cfg(test)]
mod test_support;

// Match the compiler ABI; Dylint supplies and owns the actual driver.
dylint_linting::dylint_library!();

// Dylint loads this symbol using the compiler's Rust ABI.
#[unsafe(no_mangle)]
#[allow(clippy::no_mangle_with_rust_abi)]
pub fn register_lints(_session: &rustc_session::Session, store: &mut rustc_lint::LintStore) {
    for register in rules::RULES {
        register(store);
    }
}
