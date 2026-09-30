+++
title = "Platforms and performance"
description = "What each download is, building dream_archivetool yourself and its Cargo features, how releases are signed, the license, what is tested, and what the operations cost."
weight = 80

[extra]
kind = "reference"
+++

## Downloads

| Download | For |
|---|---|
| `dream_archivetool-Windows-X64.zip` | Windows, x86-64 |
| `dream_archivetool-macOS-ARM64.zip` | macOS, Apple silicon |
| `dream_archivetool-macOS-X64.zip` | macOS, Intel |
| `dream_archivetool-Linux-X64.zip` | Linux, x86-64 |
| `dream_archivetool-Android-ARM64.zip` | Android 6 (API 23) or newer, ARM64. A program for a terminal such as Termux, not an app |
| `dream_archivetool-Portmaster-ARM64.zip` | ARM64 Linux handhelds with glibc 2.34 or newer, run from a terminal or over SSH |

Every download is the same command line, with every command. Each archive holds the program,
`dream_archivetool-README.md`, `dream_archivetool-LICENSE-MIT`, `dream_archivetool-LICENSE-APACHE`,
and a Sigstore bundle for the program, such as `dream_archivetool-Linux-X64.bundle`.

## Building it yourself

```sh
cargo install dream_archivetool
```

That builds the program. The library is the same crate; what it contains is a matter of Cargo
features:

| Feature | Default | Adds |
|---|---|---|
| `cli` | Yes | The program, with `clap`, its completions and manual page, and `serde_json` |
| `luau` | No | The [Luau extension](@/docs/luau/_index.md), and dream_archive's. `lua` is the old name |
| `luau-analysis` | No | `luau` plus l3i's type checker, for the crate's own typed tests |

The release downloads are built with the default features. The program needs `cli`, so
`cargo build --no-default-features` builds the library alone. A program that wants only the
library leaves the command line out:

```toml
[dependencies]
dream_archivetool = { version = "1", default-features = false }
```

The minimum Rust version is 1.88. Building with `luau` needs l3i's toolchain: clang, lld and
cross-language thin LTO, set up as [Embedding Luau](@/docs/luau-hosts.md#dependencies-and-toolchain)
describes. The default build needs only Rust.

## Signed releases

Each release archive holds, beside the program, a Sigstore bundle made by StroggForge's release
workflow. It proves that workflow built the program for this repository:

```sh
cosign verify-blob dream_archivetool \
  --bundle dream_archivetool-Linux-X64.bundle \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github.com/DreamWeave-MP/StroggForge/\.github/workflows/rustGlobalBuild\.yml@' \
  --certificate-github-workflow-repository DreamWeave-MP/dream_archivetool
```

```text
Verified OK
```

Each GitHub release also links every archive's VirusTotal scan.

## License

MIT OR Apache-2.0, at your option. Releases up to and including 1.0.0 were GPL-3.0-or-later.
dream_archive, which it is built on, is MIT OR Apache-2.0 as well.

## What is tested

Every push runs [StroggForge](https://github.com/DreamWeave-MP/StroggForge)'s release workflow:
the tests with every feature on Windows, Linux, and macOS on Apple silicon and Intel; Clippy at
the pedantic level with warnings as errors; `rustfmt`; and `cargo audit`, before anything is
built for release.

The tests build archives in every format and run each operation against them: extraction under
each overwrite policy, names that must be refused, non-UTF-8 names, symbolic links, compressed
and texture archives, updates that keep and replace entries, and rewrites that must be refused.
The command line is tested through the built program, down to what reaches standard output and
the exit codes. The Luau tests type-check the plan's definitions with Luau's own checker, run a
strict script against both modules, and drive every function and method.

## What the operations cost

`cargo bench --bench archive_ops` times the Rust operations on synthetic archives. The 0.2.1
release recorded these means, on a GitHub Actions runner:

| Operation | Archive | Time |
|---|---|---:|
| `list` | 256 entries | 187.3 µs |
| Open, then read one entry | 4 KiB entry | 109.6 µs |
| Read one entry from an opened archive | 4 KiB entry | 0.30 µs |
| `verify --read-payloads` | 256 entries of 4 KiB | 333.0 µs |
| `extract-all` | 192 entries of 4 KiB | 11.72 ms |
| `extract-all --skip-existing`, all present | 192 entries | 0.79 ms |
| `extract-all` | 4 entries of 8 MiB | 35.05 ms |
| `create --format tes3` | 128 entries of 4 KiB | 6.83 ms |
| `add` of one file, keeping the rest | 128 entries of 4 KiB | 4.92 ms |
| `add` of one file, keeping the rest | 2000 entries of 128 bytes | 58.93 ms |
| `add` of one texture to a texture BA2, keeping the rest | 4 textures of 1 MiB | 9.00 ms |
| `diff --hash` | 128 entries of 4 KiB | 1.74 ms |
| `diff --hash` | 2000 entries of 128 bytes | 5.26 ms |

Opening an archive is most of the cost of reading one file from it, which is why the library has
[`OpenArchive`](@/docs/api/archive-tool.md#openarchive) for repeated reads. Skipping existing
files costs almost nothing, because a skipped file is never read.

From Luau, `cargo bench --features luau --bench luau_boundary` runs frozen scripts against a
2000-entry TES3 archive opened with `openPath`. Means per call, the 0.2.1 `mlua` binding against
the l3i extension that replaced it, on one machine:

| Script | `mlua`, 0.2.1 | l3i |
|---|---:|---:|
| `planExtract` of 2000 paths | 10.0 ms | 6.5 ms |
| `planExtractAll().entries`, iterated | 6.9 ms | 6.8 ms |
| `planExtractAll().entries[i]`, indexed | 6.8 ms | 6.2 ms |
| `extractMany` of 256 entries to disk | 5.8 ms | 5.5 ms |
| `verify()` | 2.0 ms | 1.0 ms |
| `diff(other, { fingerprintPayloads = true })` | 6.5 ms | 2.5 ms |
| `diff` rows, iterated | 2.6 ms | 1.6 ms |
| `toolInfo()` | 1.2 µs | 1.0 µs |

Plans and batches are bound by the filesystem, one `stat` per target and one file per extracted
entry, so the reports show the binding's share: no row table is built until a script asks for
one.
