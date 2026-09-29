# Architecture Notes

`dream_archivetool` is a policy layer over `dream_archive`, not a second archive parser. Keep that boundary boring. The archive crate owns format mechanics; this crate owns application decisions about safety, filesystem writes, CLI shape, Luau tables, and rewrite refusal.

## Layer ownership

- `dream_path` owns virtual archive path normalization and path helper semantics.
- `dream_archive` owns BSA/BA2 parsing, writing, entry lookup, payload extraction, and lower-level Luau archive userdata.
- `dream_archivetool` owns policy: safe extraction targets, JSON/CLI/Luau DTOs, create/add planning, rewrite blockers, symlink policy, temp-output replacement, and durability options.
- `src/cli` owns argument parsing and presentation only. It should translate command-line options into library option structs, then get out of the way.
- `src/lua.rs` owns Luau embedding ergonomics and converts between Luau values and the same library policy APIs used by the CLI.

If a change needs to understand archive record layout, it probably belongs in `dream_archive`. If a change decides whether writing to the host filesystem is safe or user-friendly, it belongs here.

## Archive path identity

Display paths are for people. `path_bytes_hex` (`pathBytesHex` in Luau tables) is a hex-encoded normalized archive-path lookup key emitted by this tool. It is not raw archive-name identity: distinct raw paths can normalize to the same lookup key. Use `verify` to detect duplicate normalized paths. Use lower-level raw/index APIs when raw identity matters.

This rule applies to CLI JSON, Rust DTOs, and Luau tables. Do not add a new public path field unless it states whether it is display text, normalized lookup bytes, raw archive bytes, or a host filesystem path.

## Rewrite policy

Rewriting must fail before output is touched when the tool cannot preserve known archive semantics. Current blockers include:

- entries without recoverable paths, including TES4 hash-only entries;
- TES4 archive flags this layer cannot preserve;
- BA2 GNMF writes;
- duplicate normalized input paths;
- explicit add output equal to the source archive path.

`add` is a full rewrite, not patch-in-place mutation. When output is omitted, the rewritten archive is produced through a temporary file before replacing the source archive.

## Luau calling styles

There are two Luau policy surfaces:

- top-level `@dream/archivetool` functions take host paths and open archives for that operation;
- methods on `dream.archive.Archive` operate on the supplied `dreamArchive.open*` handle.

The methods are an l3i augmentation: `dream.archivetool` declares `requires("dream.archive")`
and adds its members to the type `dream.archive` owns. The planner merges the two declarations
into one metatable before any runtime exists, so there is no registration order to get right and
no second wrapper type. A member name both extensions declare fails the plan; coordinate names
with dream_archive.

`dreamArchive.openBytes(...)` userdata is an immutable byte snapshot. `dreamArchive.openPath(...)`
avoids a policy-layer reopen, but payload reads still follow `dream_archive` source semantics; this
layer does not promise that path-backed file bytes are pinned after handle creation.

There are intentionally no `archive:add`, `archive:planAdd`, `archive:create`, or `archive:planCreate`
methods. Creation and rewrite are host-filesystem operations with output selection, symlink policy,
and temp-file replacement semantics. They remain top-level path APIs.

## Lifetime and memory shape

`LoadedArchive` owns a `dream_archive::Archive`. `LoadedArchiveRef` is a borrowed view used so policy code can run against already-opened archives, especially Lua userdata. Keep this ownership visible. A clever trait hierarchy that hides whether an archive is owned or borrowed is not an improvement unless it also removes a real bug.

Extraction streams payloads to writers through `dream_archive`; it should not materialize whole payloads in this crate unless a public API explicitly asks for bytes in memory. Planning and reporting APIs do materialize DTO vectors by design. Their Luau forms are userdata over
one `Rc` of the DTO: rows are handles into it, and only `:toTable()` materializes Lua tables.

## Compatibility checklist

Before changing public behavior, check:

- CLI JSON fields and names; additive fields are minor-version compatible, removals or renames are not.
- Future public DTO fields that matter for deserializing saved JSON should be optional or
  `serde(default)` so older data does not fail just because a newer tool knows more facts.
- Luau function names, option keys, report and plan members, enum-like string values (all camelCase), integer `size`/`compressedSize`/`payloadFingerprint` fields, and the `:toTable()` shapes.
- Rust public DTO fields and serde names.
- Archive rewrite blockers and unsupported-format diagnostics.
- `path_bytes_hex` / `pathBytesHex` normalized lookup semantics.
- Feature matrix: default CLI, no-default library, standalone Luau tests/docs, and all-feature clippy.

The fixed limit is ugly, but it is at least a number. Replacing explicit contracts with vague convenience is how tools start corrupting user data politely.
