--!strict
--!native
-- SPDX-License-Identifier: MIT OR Apache-2.0
-- The command line's help: dream_archivetool 1.1.0's own, under the name this target runs as,
-- with what the port leaves out said where the original describes it, and the exit statuses.

local Statuses = 'Exit status: 0 done, 8 the command line can\'t be used, 9 the run failed.'

local Top = [[
dream_archivetool 1.1.0 for Jess, run here as dream-archivetool: inspect and manipulate Bethesda
BSA and BA2 archives

Usage: dream-archivetool [OPTIONS] [COMMAND]

Commands:
  info         Print archive metadata
  list         List archive entries
  verify       Verify archive structure and optional payload readability
  diff         Compare two archives by normalized path bytes
  extract      Extract one archive entry
  extract-all  Extract every archive entry
  create       Create a new archive from a file or directory
  add          Add or update entries by rewriting the archive
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

Not part of this port, and refused by name: --generate-completion and --generate-manpage, there
being no completion script or manpage to write, and --fsync on the commands that write, because
@dream/fs can't flush a file and its folder to disk.

Exit status: 0 done, 8 the command line can't be used, 9 the run failed; the original exits 2
and 1 for those. A failed run says why on standard error as `ERROR: <message>`, in the
original's words; a command line that can't be used, as `dream-archivetool: <message>`.]]

local Topics: { [string]: string } = {
  info = [[
Print archive metadata

Usage: dream-archivetool info [OPTIONS] <ARCHIVE>

Arguments:
  <ARCHIVE>  Archive path

Options:
      --json  Write JSON to stdout
  -h, --help  Print help]],

  list = [[
List archive entries

Usage: dream-archivetool list [OPTIONS] <ARCHIVE>

Arguments:
  <ARCHIVE>  Archive path

Options:
  -l, --long  Include entry sizes; JSON always includes available size fields
      --json  Write JSON to stdout
  -h, --help  Print help]],

  verify = [[
Verify archive structure and optional payload readability

Usage: dream-archivetool verify [OPTIONS] <ARCHIVE>

Arguments:
  <ARCHIVE>  Archive path

Options:
      --read-payloads  Stream every named payload to a sink
      --json           Write JSON report to stdout
  -h, --help           Print help]],

  diff = [[
Compare two archives by normalized path bytes

Usage: dream-archivetool diff [OPTIONS] <OLD> <NEW>

Arguments:
  <OLD>  Old archive path
  <NEW>  New archive path

Options:
      --hash  Hash payload bytes instead of comparing only metadata
      --json  Write JSON report to stdout
  -h, --help  Print help]],

  extract = [[
Extract one archive entry

Usage: dream-archivetool extract [OPTIONS] <ARCHIVE> [ENTRY]

Arguments:
  <ARCHIVE>  Archive path
  [ENTRY]    Entry path inside the archive. Jess hands a command only UTF-8 arguments, so a
             name that isn't UTF-8 is given with --entry-hex

Options:
      --entry-hex <HEX>  Hex-encoded normalized entry path bytes from `list --json` `path_bytes_hex`
  -o, --output <OUTPUT>  Output directory. Defaults to the current directory
      --stdout           Write file bytes to stdout
      --fsync            Not part of this port: @dream/fs can't flush a file and its folder
      --flat             Discard archive directories and write only the basename
      --overwrite        Replace existing files
      --skip-existing    Leave existing files untouched
      --json             Write JSON summary to stdout
  -h, --help             Print help]],

  ['extract-all'] = [[
Extract every archive entry

Usage: dream-archivetool extract-all [OPTIONS] <ARCHIVE>

Arguments:
  <ARCHIVE>  Archive path

Options:
  -o, --output <OUTPUT>  Output directory. Defaults to the current directory
      --overwrite        Replace existing files
      --skip-existing    Leave existing files untouched
      --json             Write JSON summary to stdout; dry-run always writes a JSON plan
      --dry-run          Print JSON extraction plan without writing files
      --fsync            Not part of this port: @dream/fs can't flush a file and its folder
  -h, --help             Print help]],

  create = [[
Create a new archive from a file or directory

Usage: dream-archivetool create [OPTIONS] --format <FORMAT> <ARCHIVE> <INPUT>

Arguments:
  <ARCHIVE>  Output archive path
  <INPUT>    Input file or directory. Directory contents are stored relative to that root

Options:
      --format <FORMAT>              Archive format to write [possible values: tes3, tes4, ba2]
      --tes4-version <TES4_VERSION>  TES4 BSA version; only valid with --format tes4 [possible values: oblivion, fallout3, skyrim, skyrim-se]
      --ba2-kind <BA2_KIND>          BA2 archive kind; only valid with --format ba2. GNMF archive writing is rejected [possible values: gnrl, dx10, gnmf]
      --ba2-version <BA2_VERSION>    BA2 version; only valid with --format ba2 [possible values: fallout4, starfield, fallout4-next-gen]
      --compress                     Enable compression for TES4 BSA and BA2 archives
      --json                         Write JSON summary to stdout; dry-run always writes a JSON plan
      --dry-run                      Print JSON archive creation plan without writing files
      --fsync                        Not part of this port: @dream/fs can't flush a file and its folder
      --follow-symlinks              Follow symbolic links while collecting input files
  -h, --help                         Print help]],

  add = [[
Add or update entries by rewriting the archive

Usage: dream-archivetool add [OPTIONS] <ARCHIVE> <INPUTS>...

Arguments:
  <ARCHIVE>    Input archive path
  <INPUTS>...  Files or directories to add. Directory contents are stored relative to each root

Options:
  -o, --output <OUTPUT>  Output archive path. Omit to replace the input archive after a successful rewrite
      --json             Write JSON summary to stdout; dry-run always writes a JSON plan
      --dry-run          Print JSON archive update plan without writing files
      --fsync            Not part of this port: @dream/fs can't flush a file and its folder
      --follow-symlinks  Follow symbolic links while collecting input files
  -h, --help             Print help]],
}

--- The help for `topic`, a subcommand's name, or the command's own without one.
local function of(topic: string?): string
  local text = if topic then Topics[topic] else nil
  if not text then return Top end

  return `{text}\n\n{Statuses}`
end

return table.freeze { of = of }
