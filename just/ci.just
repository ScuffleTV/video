export CI := "1"
export RUSTUP_TOOLCHAIN := "nightly"

test:
    cargo llvm-cov nextest --branch --lcov --profile ci --output-path ./lcov.info --features scuffle-ffmpeg/build

fmt:
    cargo fmt --check --all -- --color=always

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

hakari:
    cargo hakari generate --diff
    cargo hakari manage-deps --dry-run
