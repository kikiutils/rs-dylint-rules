use ra_ap_syntax::{
    TextRange,
    TextSize,
};

use super::{
    Api,
    analysis::{
        self,
        ApiUse,
        Kind,
    },
};
use crate::test_support::{
    apply,
    parse,
};

// Name resolution decides whether a qualified access gets a fix; the diagnostic stays either way.
#[test]
fn qualified_access_is_fixed_only_when_the_name_is_free() {
    let source = "fn run() -> anyhow::Result<()> { Ok(()) }";
    let Some(start) = source.find("anyhow::Result") else {
        panic!("missing API")
    };

    let range = TextRange::at(TextSize::of(&source[..start]), TextSize::of("anyhow::Result"));
    let fact = ApiUse {
        api: Api::Result,
        kind: Kind::Access,
    };

    let root = parse(source);
    for free in [true, false] {
        let output = analysis::analyze(&root, source, [(range, &fact)], |_| free);
        assert_eq!(output.issues.len(), 1);
        if free {
            let fixed = apply(source, &output);
            assert!(fixed.contains("use anyhow::Result;"), "{fixed}");
            assert!(fixed.contains("-> Result<()>"), "{fixed}");
        } else {
            assert!(output.issues[0].fix.is_none());
        }
    }
}
