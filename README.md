# Loon: A safe sandboxed VM for embedding software

Loon is a small garbage collected VM in the spirit of Lua, implemented in Rust
to be able to embed in larger programs with reasonable performance.

The core of this is a rust crate, however it is also intended to be used as
a statically or dynamically linked C library, using a custom allocator.

## Licensing

The code and documentation in the `loon` git repository is [free
software](https://www.gnu.org/philosophy/free-sw.html), dual-licensed
under the [MIT](./LICENSE-MIT) or [Apache-2.0](./LICENSE-APACHE)
license, at your choosing.
