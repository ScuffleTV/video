test *args:
    #!/bin/bash
    set -e -o pipefail 

    INSTA_FORCE_PASS=1 cargo +nightly llvm-cov clean --workspace
    INSTA_FORCE_PASS=1 cargo +nightly llvm-cov nextest --branch --include-build-script --no-report {{args}}

    # Do not generate the coverage report on CI
    cargo insta review
    cargo +nightly llvm-cov report --html
    cargo +nightly llvm-cov report --lcov --output-path ./lcov.info

test-ci:
    CI=1 cargo +nightly llvm-cov nextest --branch --lcov --profile ci --output-path ./lcov.info --features scuffle-ffmpeg/build

codecov-validate:
    cat codecov.yml | curl --data-binary @- https://codecov.io/validate
