# AGENTS.md

## Repository shape

- Library plus CLI: package `dream_archivetool`, edition `2024`, MSRV `1.88`, license `MIT OR Apache-2.0`.
  `src/archive.rs` is the `ArchiveTool` facade, `src/extract.rs`, `src/create.rs`, `src/diff.rs`,
  `src/verify.rs` the policy operations, `src/loaded.rs` the owned/borrowed archive views,
  `src/cli/` the binary.
- The Luau bindings (`src/luau.rs`, feature `luau`; `lua` is an alias) are an l3i extension:
  `dream.archivetool`, module `@dream/archivetool`, requiring `dream.archive`. The policy methods
  are an augmentation of `dream.archive.Archive` (`ExtensionDescriptor::augment_userdata`), never a
  second wrapper type. Reports and plans are userdata over one `Rc` of the DTO with sequence views
  of row handles; `:toTable()` gives the old table shape. Never create a VM in library code, never
  pick tags or atoms, keep every member `.signature(..)`d (fingerprints are `integer`, sizes and
  counts `number`), and keep member names distinct from dream_archive's (a clash fails the plan).
- `l3i` comes from crates.io; `dream_archive` is a path override (`../dream_archive`) during the
  migration campaign; the `luau-analysis` feature (`luau` plus `l3i/analysis`) turns on the
  definitions gate for the typed tests, since Cargo has no optional dev-dependencies.
- `.cargo/config.toml` is l3i's toolchain policy (clang++, lld, cross-language thin LTO); copy it
  from l3i verbatim when it changes. Build inside the toolchain that has clang
  (`toolbox run -c l3i-tools44 ...` on the maintainer's machine).

## Commands

- Gate before handoff/commit:
  - `cargo fmt --check`
  - `cargo test --all-features`
  - `cargo clippy --all-targets --all-features -- -W clippy::pedantic -D warnings`
  - `cargo doc --no-deps --all-features`
- The Luau surface is `cargo test --features luau --test luau_api` (behaviour) or
  `--features luau-analysis` (plus the definitions gate and the strict script); the CLI contract
  is `cargo test --test cli_contract`.
- Benches: `cargo bench --bench archive_ops` (Rust policy operations) and
  `cargo bench --bench luau_boundary --features luau` (frozen Luau scripts; the README's
  before/after table). `cargo clean` after a bench session: the l3i release build is several
  gigabytes.

## Conventions that matter

- Host paths are UTF-8 strings; archive entry paths are bytes (or entry handles); `pathBytesHex`
  is the stable normalized lookup key. Plans and batches check that every named member exists
  before anything is written.
- Option tables are read with l3i's strict `Options` (camelCase keys, unknown keys are errors).
- CLI JSON shapes are a compatibility surface separate from the Luau one; see
  `docs/architecture.md`.
