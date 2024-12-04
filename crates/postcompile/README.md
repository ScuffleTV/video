# postcompile

> [!WARNING]  
> This crate is under active development and may not be stable.

[![crates.io](https://img.shields.io/crates/v/postcompile.svg)](https://crates.io/crates/postcompile) [![docs.rs](https://img.shields.io/docsrs/postcompile)](https://docs.rs/postcompile)

---

A crate which allows you to post-compile Rust code.

This is particularly useful when making snapshot tests of proc-macros, look below for an example with the `insta` crate.

## Usage

```rs
#[test]
fn some_cool_test() {
    insta::assert_snapshot!(postcompile::compile! {
        #[derive(Debug, Clone)]
        struct Test {
            a: u32,
            b: i32,
        }

        const TEST: Test = Test { a: 1, b: 3 };
    });
}
```

## Status

This crate is currently under development and is not yet stable, unit tests are not yet fully implemented.

Unit tests are not yet fully implemented. Use at your own risk.

## License

This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
You can choose between one of them if you use this work.

`SPDX-License-Identifier: MIT OR Apache-2.0`
