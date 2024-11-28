# scuffle-h3-webtransport

> [!WARNING]  
> This crate is under active development and may not be stable.

 [![crates.io](https://img.shields.io/crates/v/scuffle-h3-webtransport.svg)](https://crates.io/crates/scuffle-h3-webtransport) [![docs.rs](https://img.shields.io/docsrs/scuffle-h3-webtransport)](https://docs.rs/scuffle-h3-webtransport)

---

A pure rust implementation of the webtransport protocol built on top of the h3 crate.

## Why?

Forked of [h3-webtransport](https://github.com/hyperium/h3/tree/master/h3-webtransport) with the following changes:

- Cleaned up logic around the upgrade handshake and the webtransport session.

We aim to merge this back into the upstream crate, but I have not finalized a design for how to do this.

Currently serves just as a proof of concept, but I have plans to use it in Scuffle.

## License

This project is licensed under the [MIT](./LICENSE.MIT) which is from the original [h3-webtransport](https://github.com/hyperium/h3/blob/master/LICENSE) crate.

`SPDX-License-Identifier: MIT`
