# Agent notes

Bugs found while documenting dream_archivetool 1.0.0 (5a9747e) for the site. The repository was
being changed by other agents at the time, so none is fixed here; the site describes each
behavior as it is. Every one reproduces with the debug build of that commit.

## `add --output NAME` fails when NAME has no folder and does not exist

```sh
dream_archivetool create MyMod.bsa MyMod --format tes3
dream_archivetool add MyMod.bsa Update --output MyMod-2.bsa
# ERROR: I/O error: No such file or directory (os error 2)
dream_archivetool add MyMod.bsa Update --output ./MyMod-2.bsa   # works
```

`reject_explicit_same_archive_output` in `src/create.rs` calls `comparable_path(output)`. For a
bare `MyMod-2.bsa` that does not exist, `path.parent()` is `Some("")`, not `None`, so the
`unwrap_or_else(|| Path::new("."))` fallback never applies and `"".canonicalize()` fails with
ENOENT. `plan_add`, `add`, and Luau's `add`/`planAdd` with `output = "fresh.bsa"` all fail the same
way. An empty parent should be treated as `.`.

## Written files are mode 0600 whatever the umask, and `add` in place narrows an archive's mode

```sh
umask 022
dream_archivetool extract MyMod.bsa readme.txt --output out && ls -l out/readme.txt   # -rw-------
dream_archivetool create New.bsa MyMod --format tes3 && ls -l New.bsa               # -rw-------
chmod 644 MyMod.bsa && dream_archivetool add MyMod.bsa Update && ls -l MyMod.bsa     # -rw-------
```

Every write goes through `tempfile::NamedTempFile`, which creates its file with mode 0600, and
`persist` keeps that mode: `write_target_with_parent_mode` / `persist_temp` in `src/extract.rs`,
and `write_temp_output` / `persist_temp_output` in `src/create/temp_output.rs`. Extracted trees
are unreadable to other users and to servers running as another user, and `add` without
`--output` silently takes group and other read access away from an existing archive. Extraction
and `create` should apply the default mode (0666 minus the umask); `add` in place should keep the
source archive's permissions.

## `add` to a compressed BA2 writes uncompressed entries

```sh
dream_archivetool create c.ba2 MyMod --format ba2 --compress
dream_archivetool list --json c.ba2          # every entry has a compressed_size
dream_archivetool add c.ba2 Update --output ./c2.ba2
dream_archivetool list --json c2.ba2         # every compressed_size is null, kept entries included
```

`write_ba2_like` in `src/create.rs` sets the version on the new `Ba2Builder` but never its
compression, so a general BA2 loses compression on every entry, the preserved ones too.
`write_dx10_ba2_like` sets compression only when the source's format is LZ4, so new textures in a
zlib DX10 BA2 are stored uncompressed (preserved textures keep their chunks). The source archive's
`compression_format` is `zip` whether or not anything is compressed, so the fix needs to look at
the entries (any compressed chunk) rather than the header alone.

## `verify` reports unsafe paths by their raw bytes, not the normalized key

An archive whose stored name is `..\..\..\c.dds`:

```json
"unsafe_paths": [
  { "path": "..\\..\\..\\c.dds", "path_bytes_hex": "2e2e5c2e2e5c2e2e5c632e646473" }
]
```

`list` reports the same entry as `../../../c.dds` / `2e2e2f...`, and the field is documented
(`VerifyPathIssue::path_bytes_hex`, `docs/architecture.md`) as the normalized lookup key.
`verify_loaded_archive` in `src/verify.rs` calls `path_issue(&entry.raw_path)`; it should pass
`entry.path` and, like the duplicate rows, put the stored bytes in `raw_path_bytes_hex`. The site
documents the current behavior and will need a line changed when this is fixed.

## A closed pipe is reported as an error with exit code 1

```sh
dream_archivetool list Morrowind.bsa | head -3
# three lines, then on stderr: ERROR: I/O error: Broken pipe (os error 32)
dream_archivetool list --json Morrowind.bsa | head -3
# ... ERROR: archive error: Broken pipe (os error 32)
```

`src/main.rs` prints every error. An `io::ErrorKind::BrokenPipe` while writing standard output
should end the program quietly with success, as other command-line tools do. The JSON writers in
`src/cli/mod.rs` also wrap the serde error as `ArchiveError::Archive`, which hides the I/O kind.

## Error messages that name another interface's option

- A symbolic link input is refused with `refusing to follow symlink input path: ...; pass
  follow_symlinks to opt in` (`src/create/input.rs`), the Rust field name, on the command line
  (`--follow-symlinks`) and in Luau (`followSymlinks`) alike.
- `CreateOptions { format: Tes3, compress: true, .. }` fails with `--compress is not valid with
  TES3 BSA archives` (`reject_unsupported_create_options`, `src/create.rs`), the command-line
  flag, from Rust and from Luau's `create`.

## Stale references

- `Cargo.toml`'s `documentation` points at
  `https://dreamweave-mp.github.io/dream_archivetool/dream_archivetool/index.html`, the rustdoc
  Pages path. With `mod_template: true` StroggForge deploys the site instead of rustdoc, so that
  page stops existing; point it at `https://dreamweave-mp.github.io/dream_archivetool/`.
- `docs/architecture.md` still says `src/lua.rs` owns the Luau embedding; it is `src/luau.rs`.
