#!/usr/bin/env bash

# shellcheck disable=SC1091

set -euo pipefail

SCRIPT_DIR="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
source "${SCRIPT_DIR}/libs/common.sh"

# Run
main() {
    prepend_cargo_bin_to_path
    require_cmd cargo
    cd -- "${REPO_ROOT}"

    log_info 'Applying Dylint fixes...'
    cargo dlint-fix --allow-dirty --allow-staged

    log_info 'Applying Clippy fixes...'
    cargo lint-fix --allow-dirty --allow-staged

    # TODO: Re-enable when Mordant supports this crate's nightly-2026-10-01 rustc_private APIs.
    # Its current driver uses nightly-2026-09-01, which cannot compile this crate.
    # log_info 'Applying Mordant fixes...'
    # cargo mlint-fix --allow-dirty --allow-staged

    log_info 'Formatting Rust sources...'
    exec cargo format-fix
}

main "$@"
