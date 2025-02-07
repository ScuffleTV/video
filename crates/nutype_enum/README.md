# nutype-enum

> [!WARNING]  
> This crate is under active development and may not be stable.

[![crates.io](https://img.shields.io/crates/v/nutype-enum.svg)](https://crates.io/crates/nutype-enum) [![docs.rs](https://img.shields.io/docsrs/nutype-enum)](https://docs.rs/nutype-enum)

---

The crate provides a macro to create a new enum type with a single field.

This is useful when you have a value and you want to have enum like behavior and have a catch all case for all other values.

# Examples

```rust
nutype_enum! {
    pub enum AacPacketType(u8) {
        SeqHdr = 0x0,
        Raw = 0x1,
    }
}
```

## License

This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
You can choose between one of them if you use this work.

`SPDX-License-Identifier: MIT OR Apache-2.0`
