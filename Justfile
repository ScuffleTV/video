mod ci 'just/ci.just'

test *args:
    #!/bin/bash
    set -euo pipefail

    # We use the nightly toolchain for coverage since it supports branch & no-coverage flags.
    export RUSTUP_TOOLCHAIN=nightly

    INSTA_FORCE_PASS=1 cargo llvm-cov clean --workspace
    INSTA_FORCE_PASS=1 cargo llvm-cov nextest --branch --include-build-script --no-report {{args}}

    # Do not generate the coverage report on CI
    cargo insta review
    cargo llvm-cov report --html
    cargo llvm-cov report --lcov --output-path ./lcov.info

hakari:
    cargo +nightly hakari generate
    cargo +nightly hakari manage-deps

clippy:
    cargo +nightly clippy --fix --allow-dirty --all-targets --all-features

fmt:
    cargo +nightly fmt --all
