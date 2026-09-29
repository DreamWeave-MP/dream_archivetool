+++
title = "Embedding Luau"
description = "Give scripts @dream/archivetool through l3i: the feature, the toolchain, composing it with dream_archive, the globals, type definitions, and what changed from 0.2."
weight = 45

[extra]
kind = "guide"
+++

With the `luau` feature, dream_archivetool is an [l3i](https://github.com/DreamWeave-MP/l3i)
extension. It describes the `@dream/archivetool` module and the methods it adds to
dream_archive's archive type, and a Rust host that runs Luau through l3i composes it, with
dream_archive's own extension, into its runtime. Scripts then run the same checks as the command
line. The crate never creates a VM and never installs a global; both are the host's decisions.

## Dependencies and toolchain

```toml
[dependencies]
dream_archivetool = { version = "1", default-features = false, features = ["luau"] }
l3i = "1"
```

`default-features = false` leaves out the command line. The `luau` feature turns on
dream_archive's `luau` feature too, and dream_archivetool re-exports the crate as
`dream_archivetool::dream_archive`, so the host needs no dependency of its own on it and cannot
end up with two copies.

l3i builds Luau itself, and only with clang, lld and cross-language thin LTO: its build script
refuses any other configuration and names the missing piece. Cargo does not pass a dependency's
configuration on, so the host copies the policy into its own `.cargo/config.toml`, as
dream_archivetool does:

```toml
[env]
CXX = "clang++"

[target.x86_64-unknown-linux-gnu]
rustflags = ["-Clinker-plugin-lto", "-Clinker=clang", "-Clink-arg=-fuse-ld=lld"]
```

clang and rustc must use the same LLVM major version. The
[l3i toolchain notes](https://github.com/DreamWeave-MP/l3i/blob/main/TOOLCHAIN.md) have the lines
for macOS and Windows, and the measurements behind the rule.

## Composing the extensions

`ArchivetoolExtension` requires dream_archive's `ArchiveExtension`; both go into the host's
`RuntimePlan`, in either order. Every runtime made from the plan can `require` both modules, and
every archive a script opens has the tool's methods:

```rust
use dream_archivetool::dream_archive::luau::ArchiveExtension;
use dream_archivetool::luau::ArchivetoolExtension;
use l3i::Runtime;
use l3i::extension::RuntimePlan;

fn main() -> l3i::Result<()> {
    let plan = RuntimePlan::builder()
        .extension(ArchiveExtension)
        .extension(ArchivetoolExtension)
        .finalize()?;
    let runtime = Runtime::from_plan(&plan)?;

    runtime.exec(r#"
        local dreamArchive = require("@dream/archive")
        local tool = require("@dream/archivetool")

        local archive = dreamArchive.openPath("MyMod.bsa")
        assert(archive:verify().rewritable)
        assert(tool.info("MyMod.bsa").fileCount == archive:len())
    "#)
}
```

Leave `ArchiveExtension` out and the plan does not finalize:
`extension 'dream.archivetool' requires 'dream.archive', which is not in the plan`.

## The dreamArchivetool global

Scripts written for 0.2 and earlier used `dreamArchive` and `dreamArchivetool` globals. A host
that still wants them exposes each module as a compatibility global through its policy; each
global and its `require` then return the same table:

```rust
use dream_archivetool::dream_archive::luau::ArchiveExtension;
use dream_archivetool::luau::{ArchivetoolExtension, MODULE};
use l3i::Runtime;
use l3i::extension::{RuntimePlan, RuntimePolicy};

fn main() -> l3i::Result<()> {
    let policy = RuntimePolicy::new()
        .compat_global(dream_archivetool::dream_archive::luau::MODULE, "dreamArchive")
        .compat_global(MODULE, "dreamArchivetool");
    let plan = RuntimePlan::builder()
        .policy(policy)
        .extension(ArchiveExtension)
        .extension(ArchivetoolExtension)
        .finalize()?;
    let runtime = Runtime::from_plan(&plan)?;

    runtime.exec(r#"
        assert(dreamArchivetool == require("@dream/archivetool"))
        assert(dreamArchive == require("@dream/archive"))
    "#)
}
```

`MODULE` is `"@dream/archivetool"`; dream_archive's is `"@dream/archive"`.

## Types for editors and checks

Every function, method and field carries a Luau signature. `plan.type_definitions()` returns the
`.d.luau` text for everything in the plan, both extensions included, ready to save for an
editor's language server:

```rust
use dream_archivetool::dream_archive::luau::ArchiveExtension;
use dream_archivetool::luau::ArchivetoolExtension;
use l3i::extension::RuntimePlan;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan = RuntimePlan::builder()
        .extension(ArchiveExtension)
        .extension(ArchivetoolExtension)
        .finalize()?;
    std::fs::write("dream.d.luau", plan.type_definitions())?;
    Ok(())
}
```

With l3i's `analysis` feature, `plan.check_definitions()` type-checks those definitions with
Luau's own checker. dream_archivetool's tests run it under the `luau-analysis` feature
(`cargo test --features luau-analysis`), and type-check a strict script that uses every report,
plan and view without `:toTable()`. The [type definitions](@/docs/luau/types.md) page shows what
the plan generates for this extension.

## What scripts get

- **The same checks.** Every name, target and rewrite is checked as on the command line, before
  anything is written.
- **The disk, as the host's process sees it.** The functions read and write host paths with the
  host's permissions; the extension asks for no l3i capabilities. A host that runs untrusted
  scripts should not give them this extension.
- **Results that cost what they are used for.** Reports and plans are one copy of the Rust
  result; a row exists when a script reads it, and tables only when it calls `:toTable()`.
- **Strict options.** A misspelled or `snake_case` option is an error that lists the right names.
- **UTF-8 host paths, byte-string entries.** See the [conventions](@/docs/luau/_index.md#conventions).

## From 0.2

`dream_archivetool::lua`, `create_dream_archive_module`, `register` and the `standalone-lua`
feature are gone, with `mlua`. Compose both extensions as above; the global names, function
names and option keys are the same. What scripts see differently:

- The policy methods are members of every `dream.archive.Archive`, with no separate module to
  create first.
- Reports and plans are userdata. Their fields read as before, and their lists take `#`, `[i]`
  and `for`, but not `ipairs`, `pairs` or `table.insert`; `:toTable()` gives the 0.2 table.
- `size` and `compressedSize` are integers, not decimal strings, and `payloadFingerprint` is an
  integer, not a hex string, in `:toTable()` too.
- An unknown option reads `extract: unknown option 'overwirte'; known options are ...`, and a
  wrong option type names the field: `add.output: expected string, got number`.
- `extract`, `extractMany`, `planExtract` and their `ByPathHex` forms fail with
  `archive entry not found` for an entry the archive does not have, before writing anything or
  planning the rest.
