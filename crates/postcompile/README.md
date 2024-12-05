# postcompile

> [!WARNING]  
> This crate is under active development and may not be stable.

[![crates.io](https://img.shields.io/crates/v/postcompile.svg)](https://crates.io/crates/postcompile) [![docs.rs](https://img.shields.io/docsrs/postcompile)](https://docs.rs/postcompile)

---

A crate which allows you to compile Rust code at runtime (hence the name `postcompile`).

What that means is that you can provide the input to `rustc` and then get back the expanded output, compiler errors, warnings, etc.

This is particularly useful when making snapshot tests of proc-macros, look below for an example with the `insta` crate.

## Usage

```rs
#[test]
fn some_cool_test() {
    insta::assert_snapshot!(postcompile::compile! {
        #![allow(unused)]

        #[derive(Debug, Clone)]
        struct Test {
            a: u32,
            b: i32,
        }

        const TEST: Test = Test { a: 1, b: 3 };
    });
}

#[test]
fn some_cool_test_extern() {
    insta::assert_snapshot!(postcompile::compile_str!(include_str!("some_file.rs")));
}
```

## Features

- Cached builds: This crate reuses the cargo build cache of the original crate so that only the contents of the macro are compiled & not any additional dependencies.
- Coverage: This crate works with [`cargo-llvm-cov`](https://crates.io/crates/cargo-llvm-cov) out of the box, which allows you to instrument the proc-macro expansion.

## Alternatives

- [`compiletest_rs`](https://crates.io/crates/compiletest_rs): This crate is used by the Rust compiler team to test the compiler itself. Not really useful for proc-macros.
- [`trybuild`](https://crates.io/crates/trybuild): This crate is an all-in-one solution for testing proc-macros, with built in snapshot testing.
- [`ui_test`](https://crates.io/crates/ui_test): Similar to `trybuild` with a slightly different API & used by the Rust compiler team to test the compiler itself.

### Differences

The other libraries are focused on testing & have built in test harnesses. This crate takes a step back and allows you to compile without a testing harness. This has the advantage of being more flexible, and allows you to use whatever testing framework you want.

In the examples above I showcase how to use this crate with the `insta` crate for snapshot testing.

## Status

This crate is currently under development and is not yet stable, unit tests are not yet fully implemented.

Unit tests are not yet fully implemented. Use at your own risk.

## Limitations

Please note that this crate does not work inside a running compiler process (inside a proc-macro) without hacky workarounds and complete build-cache invalidation.

This is because `cargo` holds a lock on the build directory and that if we were to compile inside a proc-macro we would recursively invoke the compiler.

## License

This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
You can choose between one of them if you use this work.

`SPDX-License-Identifier: MIT OR Apache-2.0`
