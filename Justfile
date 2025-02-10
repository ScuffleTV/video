mod? local

# By default we use the nightly toolchain, however you can override this by setting the RUST_TOOLCHAIN environment variable.
export RUST_TOOLCHAIN := env_var_or_default('RUST_TOOLCHAIN', 'nightly')

# An alias for cargo xtask check
powerset *args:
    cargo +{{RUST_TOOLCHAIN}} xtask powerset {{args}}

# An alias for cargo fmt --all
fmt *args:
    cargo +{{RUST_TOOLCHAIN}} fmt --all {{args}}

lint *args:
    cargo +{{RUST_TOOLCHAIN}} clippy --fix --allow-dirty --allow-staged --all-features --all-targets {{args}}

test *args:
    #!/bin/bash
    set -euo pipefail

    INSTA_FORCE_PASS=1 cargo +{{RUST_TOOLCHAIN}} llvm-cov clean --workspace
    INSTA_FORCE_PASS=1 cargo +{{RUST_TOOLCHAIN}} llvm-cov nextest --include-build-script --no-report --all-features -- {{args}}
    # Coverage for doctests is currently broken in llvm-cov.
    # Once it fully works we can add the `--doctests` flag to the test and report command again.
    cargo +{{RUST_TOOLCHAIN}} llvm-cov test --doc --no-report --all-features -- {{args}}

    # Do not generate the coverage report on CI
    cargo insta review
    cargo +{{RUST_TOOLCHAIN}} llvm-cov report --lcov --output-path ./lcov.info
    cargo +{{RUST_TOOLCHAIN}} llvm-cov report --html

    # Run tests under valgrind, atm just testing amf0 results in 2 errors
    echo "Running tests under Valgrind...\n"
    for test in $(find target/llvm-cov-target/debug/deps -maxdepth 1 -type f -executable | grep scuffle_amf0); do
        echo "Running ${test} under Valgrind...\n"
        valgrind --leak-check=full --error-exitcode=1 "${test}"
    done

alias docs := doc
doc *args:
    # `--cfg docsrs` enables us to write feature hints in the form of `#[cfg_attr(docsrs, doc(cfg(feature = "some-feature")))]`
    # `--enable-index-page` makes the command generate an index page which lists all crates (unstable)
    # `-D warnings` disallow all warnings
    # `-Zunstable-options` enables unstable options (for the `--enable-index-page` flag)
    RUSTDOCFLAGS="-D warnings --cfg docsrs --enable-index-page -Zunstable-options" cargo +{{RUST_TOOLCHAIN}} doc --no-deps --all-features {{args}}

alias docs-serve := doc-serve
doc-serve: doc
    miniserve target/doc --index index.html --port 3000

deny *args:
    cargo +{{RUST_TOOLCHAIN}} deny {{args}} --all-features check

workspace-hack:
    cargo +{{RUST_TOOLCHAIN}} hakari manage-deps
    cargo +{{RUST_TOOLCHAIN}} hakari generate
