mod analysis;
mod compiler;

pub(super) const RULE: fn(&mut rustc_lint::LintStore) = self::compiler::register;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Api {
    Crate,
    Anyhow,
    Bail,
    Ensure,
    Result,
    Error,
}

impl Api {
    fn name(self) -> &'static str {
        match self {
            Self::Crate | Self::Anyhow => "anyhow",
            Self::Bail => "bail",
            Self::Ensure => "ensure",
            Self::Result => "Result",
            Self::Error => "AnyhowError",
        }
    }

    fn allows(self, name: &str) -> bool {
        name == self.name() || (self == Self::Result && name == "AnyhowResult")
    }
}
