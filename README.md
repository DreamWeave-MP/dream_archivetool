# dream_archivetool

Inspect, extract, create and update Bethesda BSA and BA2 archives.

Bethesda's games keep their assets in archives: `.bsa` from Morrowind to Skyrim Special Edition,
`.ba2` from Fallout 4 on. dream_archivetool lists and checks what is inside one, extracts it, packs
folders into new ones, and adds files to existing ones. Every name and every target is checked
before anything is written, nothing you already have is replaced unless you say so, and an archive
it cannot rewrite without losing something is refused before the file is touched. Every command
that writes can print its plan first.

It is a command line, a Rust library, and a Luau extension that adds its operations to
[dream_archive](https://dreamweave-mp.github.io/dream_archive/)'s archives. dream_archive reads and
writes the formats; this crate decides what is safe to do with them.

**Documentation, downloads and the full Rust and Luau API reference:
<https://dreamweave-mp.github.io/dream_archivetool/>**

## Install

Download the build for your system from the
[releases](https://github.com/DreamWeave-MP/dream_archivetool/releases): Windows, macOS, Linux,
Android and PortMaster. Or build it:

```sh
cargo install dream_archivetool
```

## Use

```sh
dream_archivetool info Morrowind.bsa
dream_archivetool list --json Morrowind.bsa
dream_archivetool verify Morrowind.bsa --read-payloads
dream_archivetool extract Morrowind.bsa 'Icons\Tx_GoldIcon.dds' --output out
dream_archivetool extract-all Morrowind.bsa --output out --dry-run
dream_archivetool create MyMod.bsa MyMod --format tes3
dream_archivetool add MyMod.bsa MyModUpdate
```

## As a library

```toml
[dependencies]
dream_archivetool = { version = "1", default-features = false }
```

```rust
use dream_archivetool::{ArchiveTool, ExtractAllOptions, OverwriteMode};

let options = ExtractAllOptions {
    output: Some("out".into()),
    overwrite: OverwriteMode::Skip,
    ..ExtractAllOptions::default()
};
let summary = ArchiveTool::extract_all("Morrowind.bsa", &options)?;
```

With the `luau` feature, `dream_archivetool::luau::ArchivetoolExtension` is an
[l3i](https://github.com/DreamWeave-MP/l3i) extension providing the module `@dream/archivetool`
and adding `verify`, `diff`, `extract` and the rest to every `dream.archive.Archive`. The host
composes it with dream_archive's extension; the crate never creates a VM or installs a global.

## Where to read next

- [Start here](https://dreamweave-mp.github.io/dream_archivetool/docs/start-here/): look inside an
  archive, take files out, pack a folder in
- [Extracting](https://dreamweave-mp.github.io/dream_archivetool/docs/extracting/) and
  [Creating and updating](https://dreamweave-mp.github.io/dream_archivetool/docs/creating/): what
  each command checks, writes and refuses
- [Command line](https://dreamweave-mp.github.io/dream_archivetool/docs/cli/) and
  [JSON output](https://dreamweave-mp.github.io/dream_archivetool/docs/json/)
- [Rust API](https://dreamweave-mp.github.io/dream_archivetool/docs/api/),
  [Luau API](https://dreamweave-mp.github.io/dream_archivetool/docs/luau/) and
  [Embedding Luau](https://dreamweave-mp.github.io/dream_archivetool/docs/luau-hosts/), including
  what changed from the 0.2 `mlua` binding
- [Changelog](https://dreamweave-mp.github.io/dream_archivetool/home/changelog/)

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -W clippy::pedantic -D warnings
cargo test --all-features
cargo test --no-default-features
cargo doc --no-deps --all-features
```

The `luau` feature needs l3i's toolchain, clang, lld and cross-language thin LTO, which
`.cargo/config.toml` sets; the test-only `luau-analysis` feature adds l3i's analysis frontend for
the typed tests in `tests/luau_api.rs`, so `cargo test --features luau` builds without it.
`cargo bench --bench archive_ops` times the Rust operations and prints each one's peak heap
growth; `cargo bench --bench luau_boundary --features luau` times frozen Luau scripts.
[`docs/architecture.md`](docs/architecture.md) says which layer owns what.

The site in `content/` is a
[DreamWeave Mod Template](https://github.com/DreamWeave-MP/DreamWeave-Mod-Template) site; preview it
with `zola serve`.

## MSRV and license

Rust 1.88. dream_archivetool is licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option. Releases up to and including 1.0.0 were
GPL-3.0-or-later.

## Support

Has dream_archivetool been useful to you? Consider
[amplifying the signal](https://ko-fi.com/magicaldave) through ko-fi.
