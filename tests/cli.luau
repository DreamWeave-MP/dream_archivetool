--!strict
--!native
-- `@dream-archivetool/cli`: the argument lists `parse` reads into requests and the ones it
-- refuses; what `render` prints for a result built by hand; the status and the message a result
-- ends with; and whole runs through `execute`, on archives made in the harness's `scratchRoot`.
-- The expected text is dream_archivetool's own output, copied from runs of the program on the
-- same inputs. Where the installed program is older than the 1.1.0 this package follows and
-- prints something 1.1.0 changed, the text is 1.1.0's, and the case says so.
local archive = require '@dream-archive'
local bytes = require '@dream/bytes'
local cli = require '@dream-archivetool/cli'
local commands = require '@dream-archivetool/commands'
local fs = require '@dream/fs'
local test = require '@jess/test'
local tool = require '@dream-archivetool'
local expect = test.expect
local suite = test.new()

local NoSync =
  '--fsync is not part of this port: @dream/fs can\'t flush a file and its folder to disk; without it a file is still written whole beside its target and renamed into place'

--- How every subcommand's help ends.
local Statuses = '\n\nExit status: 0 done, 8 the command line can\'t be used, 9 the run failed.\n'

local ExampleHex = '74657874757265732f6578616d706c652e646473'
local MeshHex = '6d65736865732f6578616d706c652e6e6966'
local ReadmeHex = '726561646d652e747874'
local Mesh = 'mesh mesh mesh mesh mesh mesh mesh mesh'

-- Each argument list `parse` reads, and the command it is.
local Read: { { argv: { string }, command: cli.Command } } = {
  { argv = {}, command = { action = 'help' } },
  { argv = { 'help' }, command = { action = 'help' } },
  { argv = { '--help' }, command = { action = 'help' } },
  { argv = { '-h' }, command = { action = 'help' } },
  { argv = { 'help', 'help' }, command = { action = 'help' } },
  { argv = { 'help', 'extract-all' }, command = { action = 'help', topic = 'extract-all' } },
  { argv = { 'info', '--help' }, command = { action = 'help', topic = 'info' } },
  { argv = { 'create', '-h' }, command = { action = 'help', topic = 'create' } },
  { argv = { 'list', '-lh', 'a.bsa' }, command = { action = 'help', topic = 'list' } },
  { argv = { '--version' }, command = { action = 'version' } },
  { argv = { '-V' }, command = { action = 'version' } },
  {
    argv = { 'info', 'a.bsa' },
    command = { action = 'info', request = { archive = 'a.bsa' }, options = {} },
  },
  {
    argv = { 'info', '--json', 'a.bsa' },
    command = { action = 'info', request = { archive = 'a.bsa' }, options = { json = true } },
  },
  {
    argv = { 'info', '--', '--json' },
    command = { action = 'info', request = { archive = '--json' }, options = {} },
  },
  {
    argv = { 'list', 'a.bsa', '-l' },
    command = { action = 'list', request = { archive = 'a.bsa' }, options = { long = true } },
  },
  {
    argv = { 'list', '--long', 'a.bsa' },
    command = { action = 'list', request = { archive = 'a.bsa' }, options = { long = true } },
  },
  {
    argv = { 'list', 'a.bsa', '--json' },
    command = { action = 'list', request = { archive = 'a.bsa' }, options = { json = true } },
  },
  {
    argv = { 'verify', 'a.bsa', '--read-payloads', '--json' },
    command = {
      action = 'verify',
      request = { archive = 'a.bsa', readPayloads = true },
      options = { json = true },
    },
  },
  {
    argv = { 'diff', 'old.bsa', '--hash', 'new.bsa' },
    command = {
      action = 'diff',
      request = { old = 'old.bsa', new = 'new.bsa', fingerprintPayloads = true },
      options = {},
    },
  },
  {
    argv = { 'extract', 'a.bsa', 'textures/a.dds' },
    command = {
      action = 'extract',
      request = { archive = 'a.bsa', entry = 'textures/a.dds' },
      options = {},
    },
  },
  {
    argv = { 'extract', '-o', 'out', '--flat', 'a.bsa', '--overwrite', 'textures/a.dds', '--json' },
    command = {
      action = 'extract',
      request = {
        archive = 'a.bsa',
        entry = 'textures/a.dds',
        output = 'out',
        flat = true,
        overwrite = 'overwrite',
      },
      options = { json = true },
    },
  },
  {
    argv = { 'extract', 'a.bsa', 'textures/a.dds', '--skip-existing', '--output=out dir' },
    command = {
      action = 'extract',
      request = {
        archive = 'a.bsa',
        entry = 'textures/a.dds',
        output = 'out dir',
        overwrite = 'skip',
      },
      options = {},
    },
  },
  {
    argv = { 'extract', 'a.bsa', '--entry-hex', '00ff', '--stdout' },
    command = {
      action = 'extract',
      request = { archive = 'a.bsa', entryHex = '00ff', toMemory = true },
      options = {},
    },
  },
  {
    argv = { 'extract', 'a.bsa', '--entry-hex=00ff', '-oout' },
    command = {
      action = 'extract',
      request = { archive = 'a.bsa', entryHex = '00ff', output = 'out' },
      options = {},
    },
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '-o=out' },
    command = {
      action = 'extract',
      request = { archive = 'a.bsa', entry = 'a.dds', output = 'out' },
      options = {},
    },
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--output=' },
    command = {
      action = 'extract',
      request = { archive = 'a.bsa', entry = 'a.dds', output = '' },
      options = {},
    },
  },
  {
    argv = { 'extract', '--stdout', '--', 'a.bsa', '-weird.dds' },
    command = {
      action = 'extract',
      request = { archive = 'a.bsa', entry = '-weird.dds', toMemory = true },
      options = {},
    },
  },
  {
    argv = { 'extract', 'a.bsa', '-' },
    command = { action = 'extract', request = { archive = 'a.bsa', entry = '-' }, options = {} },
  },
  {
    argv = { 'extract', 'a.bsa', 'textures/invalid-\255.dds' },
    command = {
      action = 'extract',
      request = { archive = 'a.bsa', entry = 'textures/invalid-\255.dds' },
      options = {},
    },
  },
  {
    argv = { 'extract-all', 'a.bsa' },
    command = { action = 'extract-all', request = { archive = 'a.bsa' }, options = {} },
  },
  {
    argv = { 'extract-all', 'a.bsa', '-o', 'out', '--skip-existing', '--dry-run', '--json' },
    command = {
      action = 'extract-all',
      request = { archive = 'a.bsa', output = 'out', overwrite = 'skip', dryRun = true },
      options = { json = true },
    },
  },
  {
    argv = { 'extract-all', '--overwrite', 'a.bsa', '--output', 'out' },
    command = {
      action = 'extract-all',
      request = { archive = 'a.bsa', output = 'out', overwrite = 'overwrite' },
      options = {},
    },
  },
  {
    argv = { 'create', 'out.bsa', 'in', '--format', 'tes3' },
    command = {
      action = 'create',
      request = { archive = 'out.bsa', input = 'in', format = 'tes3' },
      options = {},
    },
  },
  {
    argv = {
      'create',
      '--format=tes4',
      'out.bsa',
      '--tes4-version',
      'skyrim-se',
      '--compress',
      'in',
      '--follow-symlinks',
      '--dry-run',
      '--json',
    },
    command = {
      action = 'create',
      request = {
        archive = 'out.bsa',
        input = 'in',
        format = 'tes4',
        tes4Version = 'skyrimSe',
        compress = true,
        followSymlinks = true,
        dryRun = true,
      },
      options = { json = true },
    },
  },
  {
    argv = {
      'create',
      'out.ba2',
      'in',
      '--format',
      'ba2',
      '--ba2-kind',
      'dx10',
      '--ba2-version=fallout4-next-gen',
    },
    command = {
      action = 'create',
      request = {
        archive = 'out.ba2',
        input = 'in',
        format = 'ba2',
        ba2Kind = 'dx10',
        ba2Version = 'fallout4NextGen',
      },
      options = {},
    },
  },
  {
    -- Whether a format takes an option is the run's to say, as it is the original's.
    argv = { 'create', 'out.bsa', 'in', '--format', 'tes3', '--compress', '--ba2-kind', 'gnmf' },
    command = {
      action = 'create',
      request = {
        archive = 'out.bsa',
        input = 'in',
        format = 'tes3',
        compress = true,
        ba2Kind = 'gnmf',
      },
      options = {},
    },
  },
  {
    argv = { 'add', 'a.bsa', 'one' },
    command = { action = 'add', request = { archive = 'a.bsa', inputs = { 'one' } }, options = {} },
  },
  {
    argv = { 'add', 'a.bsa', 'one', '-o', 'out.bsa', 'two', '--dry-run', '--follow-symlinks', '--json' },
    command = {
      action = 'add',
      request = {
        archive = 'a.bsa',
        inputs = { 'one', 'two' },
        output = 'out.bsa',
        followSymlinks = true,
        dryRun = true,
      },
      options = { json = true },
    },
  },
}

-- Each argument list `parse` refuses, and what it says of it.
local Refused: { { argv: { string }, message: string } } = {
  { argv = { 'bogus' }, message = 'unknown command \'bogus\'' },
  { argv = { 'Info', 'a.bsa' }, message = 'unknown command \'Info\'' },
  { argv = { 'extract_all', 'a.bsa' }, message = 'unknown command \'extract_all\'' },
  { argv = { '--bogus' }, message = 'unknown option \'--bogus\'' },
  { argv = { '-x' }, message = 'unknown option \'-x\'' },
  { argv = { '--json', 'info', 'a.bsa' }, message = 'unknown option \'--json\'' },
  {
    argv = { '--generate-completion', 'bash' },
    message = '--generate-completion is not part of this port: it has no shell completion scripts to generate',
  },
  {
    argv = { '--generate-completion=zsh' },
    message = '--generate-completion is not part of this port: it has no shell completion scripts to generate',
  },
  {
    argv = { '--generate-manpage' },
    message = '--generate-manpage is not part of this port: it has no manpage to generate',
  },
  {
    argv = { 'info', 'a.bsa', '--generate-manpage' },
    message = '--generate-manpage is not part of this port: it has no manpage to generate',
  },
  { argv = { 'help', 'bogus' }, message = 'unknown command \'bogus\'' },
  { argv = { 'help', 'info', 'list' }, message = 'help takes one command, not \'list\' as well' },
  { argv = { 'help', '--json' }, message = 'unknown option \'--json\'' },
  { argv = { 'info' }, message = 'info requires <ARCHIVE>' },
  { argv = { 'info', '--json' }, message = 'info requires <ARCHIVE>' },
  { argv = { 'info', 'a.bsa', 'b.bsa' }, message = 'unexpected argument \'b.bsa\'' },
  { argv = { 'info', 'a.bsa', '--bogus' }, message = 'unknown option \'--bogus\'' },
  { argv = { 'info', 'a.bsa', '-V' }, message = 'unknown option \'-V\'' },
  { argv = { 'info', 'a.bsa', '--long' }, message = 'unknown option \'--long\'' },
  { argv = { 'info', 'a.bsa', '--json', '--json' }, message = '--json can be given only once' },
  { argv = { 'info', 'a.bsa', '--json=1' }, message = '--json takes no value' },
  { argv = { 'list', 'a.bsa', '--json', '--long' }, message = '--long can\'t be used with --json' },
  { argv = { 'list', 'a.bsa', '-l', '--long' }, message = '--long can be given only once' },
  { argv = { 'list', 'a.bsa', '-lx' }, message = 'unknown option \'-x\'' },
  { argv = { 'verify', 'a.bsa', '--hash' }, message = 'unknown option \'--hash\'' },
  { argv = { 'diff' }, message = 'diff requires <OLD> and <NEW>' },
  { argv = { 'diff', 'old.bsa' }, message = 'diff requires <NEW>' },
  { argv = { 'diff', 'old.bsa', 'new.bsa', 'more.bsa' }, message = 'unexpected argument \'more.bsa\'' },
  { argv = { 'extract' }, message = 'extract requires <ARCHIVE>' },
  { argv = { 'extract', 'a.bsa' }, message = 'extract requires [ENTRY] or --entry-hex <HEX>' },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--entry-hex', '00' },
    message = '--entry-hex can\'t be used with [ENTRY]',
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--stdout', '-o', 'out' },
    message = '--output can\'t be used with --stdout',
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--stdout', '--flat' },
    message = '--stdout can\'t be used with --flat',
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--overwrite', '--stdout' },
    message = '--stdout can\'t be used with --overwrite',
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--stdout', '--skip-existing' },
    message = '--stdout can\'t be used with --skip-existing',
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--stdout', '--json' },
    message = '--json can\'t be used with --stdout',
  },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--overwrite', '--skip-existing' },
    message = '--overwrite can\'t be used with --skip-existing',
  },
  { argv = { 'extract', 'a.bsa', 'a.dds', '-o' }, message = '-o requires a value' },
  {
    argv = { 'extract', 'a.bsa', 'a.dds', '--output', '--json' },
    message = '--output requires a value',
  },
  { argv = { 'extract', 'a.bsa', 'a.dds', '--entry-hex' }, message = '--entry-hex requires a value' },
  { argv = { 'extract', 'a.bsa', 'a.dds', 'b.dds' }, message = 'unexpected argument \'b.dds\'' },
  { argv = { 'extract', 'a.bsa', 'a.dds', '--dry-run' }, message = 'unknown option \'--dry-run\'' },
  { argv = { 'extract', 'a.bsa', 'a.dds', '--fsync' }, message = NoSync },
  { argv = { 'extract', 'a.bsa', 'a.dds', '--stdout', '--fsync' }, message = NoSync },
  { argv = { 'extract-all', 'a.bsa', '--fsync' }, message = NoSync },
  { argv = { 'create', 'out.bsa', 'in', '--format', 'tes3', '--fsync' }, message = NoSync },
  { argv = { 'add', 'a.bsa', 'in', '--fsync' }, message = NoSync },
  {
    argv = { 'extract-all', 'a.bsa', '--skip-existing', '--overwrite' },
    message = '--overwrite can\'t be used with --skip-existing',
  },
  { argv = { 'extract-all', 'a.bsa', 'a.dds' }, message = 'unexpected argument \'a.dds\'' },
  { argv = { 'extract-all', 'a.bsa', '--flat' }, message = 'unknown option \'--flat\'' },
  { argv = { 'create', 'out.bsa', 'in' }, message = 'create requires --format <FORMAT>' },
  { argv = { 'create', 'out.bsa', '--format', 'tes3' }, message = 'create requires <INPUT>' },
  { argv = { 'create', '--format', 'tes3' }, message = 'create requires <ARCHIVE> and <INPUT>' },
  { argv = { 'create', 'out.bsa', 'in', '--format' }, message = '--format requires a value' },
  {
    argv = { 'create', 'out.bsa', 'in', '--format', 'zip' },
    message = '--format takes tes3, tes4, ba2, not \'zip\'',
  },
  {
    argv = { 'create', 'out.bsa', 'in', '--format', 'TES3' },
    message = '--format takes tes3, tes4, ba2, not \'TES3\'',
  },
  {
    argv = { 'create', 'out.bsa', 'in', '--format', 'tes4', '--tes4-version', 'morrowind' },
    message = '--tes4-version takes oblivion, fallout3, skyrim, skyrim-se, not \'morrowind\'',
  },
  {
    argv = { 'create', 'out.ba2', 'in', '--format', 'ba2', '--ba2-kind=general' },
    message = '--ba2-kind takes gnrl, dx10, gnmf, not \'general\'',
  },
  {
    argv = { 'create', 'out.ba2', 'in', '--format', 'ba2', '--ba2-version', 'fallout4NextGen' },
    message = '--ba2-version takes fallout4, starfield, fallout4-next-gen, not \'fallout4NextGen\'',
  },
  {
    argv = { 'create', 'out.bsa', 'in', '--format', 'tes3', '--format', 'tes4' },
    message = '--format can be given only once',
  },
  { argv = { 'add', 'a.bsa' }, message = 'add requires <INPUTS>...' },
  { argv = { 'add' }, message = 'add requires <ARCHIVE> and <INPUTS>...' },
  { argv = { 'add', 'a.bsa', 'in', '--format', 'tes3' }, message = 'unknown option \'--format\'' },
}

local InfoJson = [=[
{
  "path": "t3.bsa",
  "format": "tes3",
  "file_count": 3,
  "named_entry_count": 3,
  "has_unnameable_entries": false,
  "rewritable": true
}
]=]

local Tes4InfoJson = [=[
{
  "path": "t4c.bsa",
  "format": "tes4",
  "file_count": 3,
  "named_entry_count": 3,
  "has_unnameable_entries": false,
  "rewritable": true,
  "tes4": {
    "version": "v105",
    "archive_types": "ArchiveTypes(256)",
    "archive_types_bits": 256,
    "archive_flags": [
      "directory-strings",
      "file-strings",
      "compressed"
    ],
    "archive_flags_bits": 7,
    "unsupported_archive_flags_bits": 0,
    "name_mode": "strings"
  }
}
]=]

local HashOnlyInfoJson = [=[
{
  "path": "hash.bsa",
  "format": "tes4",
  "file_count": 1,
  "named_entry_count": 0,
  "has_unnameable_entries": true,
  "rewritable": false,
  "rewrite_blocker": "archive contains entries without recoverable paths; refusing to rewrite it lossy",
  "tes4": {
    "version": "v104",
    "archive_types": "ArchiveTypes(256)",
    "archive_types_bits": 256,
    "archive_flags": [],
    "archive_flags_bits": 0,
    "unsupported_archive_flags_bits": 0,
    "name_mode": "hash-only"
  }
}
]=]

local Ba2InfoJson = [=[
{
  "path": "gc.ba2",
  "format": "ba2",
  "file_count": 3,
  "named_entry_count": 3,
  "has_unnameable_entries": false,
  "rewritable": true,
  "ba2": {
    "version": "v2",
    "payload_format": "gnrl",
    "compression_format": "zip",
    "strings": true
  }
}
]=]

local ListJson = [=[
[
  {
    "path": "readme.txt",
    "path_bytes_hex": "726561646d652e747874",
    "size": 6,
    "compressed_size": 14
  },
  {
    "path": "meshes/example.nif",
    "path_bytes_hex": "6d65736865732f6578616d706c652e6e6966",
    "size": 39,
    "compressed_size": null
  },
  {
    "path": "textures/example.dds",
    "path_bytes_hex": "74657874757265732f6578616d706c652e646473",
    "size": null,
    "compressed_size": null
  }
]
]=]

-- A name that isn't UTF-8, and one with a quote, a backslash's separator, and a tab in it.
local OddListJson = [=[
[
  {
    "path": "textures/invalid-]=] .. '\239\191\189' .. [=[.dds",
    "path_bytes_hex": "74657874757265732f696e76616c69642dff2e646473",
    "size": 5,
    "compressed_size": null
  },
  {
    "path": "quote\"back/slash/tab\there.txt",
    "path_bytes_hex": "71756f7465226261636b2f736c6173682f74616209686572652e747874",
    "size": 7,
    "compressed_size": null
  }
]
]=]

local DuplicateVerifyJson = [=[
{
  "path": "dup.bsa",
  "format": "tes3",
  "file_count": 2,
  "named_entry_count": 2,
  "unnameable_entries": 0,
  "rewritable": true,
  "duplicate_normalized_paths": [
    {
      "path": "textures/example.dds",
      "path_bytes_hex": "74657874757265732f6578616d706c652e646473",
      "raw_path_bytes_hex": "74657874757265732f6578616d706c652e646473",
      "colliding_raw_path_bytes_hex": "54657874757265732f4578616d706c652e646473"
    }
  ],
  "unsafe_paths": [],
  "payloads_read": null,
  "warnings": [
    "archive contains duplicate normalized paths",
    "payload read verification skipped because duplicate normalized paths prevent per-entry coverage"
  ]
}
]=]

-- dream_archivetool 1.1.0's: the unsafe path by its normalized key, beside the stored bytes.
local UnsafeVerifyJson = [=[
{
  "path": "unsafe.bsa",
  "format": "tes3",
  "file_count": 2,
  "named_entry_count": 2,
  "unnameable_entries": 0,
  "rewritable": true,
  "duplicate_normalized_paths": [],
  "unsafe_paths": [
    {
      "path": "../../c.dds",
      "path_bytes_hex": "2e2e2f2e2e2f632e646473",
      "raw_path_bytes_hex": "2e2e5c2e2e5c432e646473"
    }
  ],
  "payloads_read": 2,
  "warnings": [
    "archive contains paths unsafe to extract directly"
  ]
}
]=]

local HashOnlyVerify = [=[
format: tes4
files: 1
named: 0
unnameable: 1
rewritable: false
rewrite blocker: archive contains entries without recoverable paths; refusing to rewrite it lossy
payloads read: 0
warning: archive contains entries without recoverable path names
]=]

local DiffJson = [=[
{
  "old": "t3.bsa",
  "new": "t3b.bsa",
  "comparison": "payload-fingerprint",
  "fingerprint_payloads": true,
  "added": [
    {
      "path": "added.txt",
      "path_bytes_hex": "61646465642e747874",
      "size": 5,
      "compressed_size": null,
      "payload_fingerprint": "235de2fdfd542145"
    }
  ],
  "removed": [
    {
      "path": "meshes/example.nif",
      "path_bytes_hex": "6d65736865732f6578616d706c652e6e6966",
      "size": 39,
      "compressed_size": null,
      "payload_fingerprint": "332292f6e742883f"
    }
  ],
  "changed": [
    {
      "path": "textures/example.dds",
      "path_bytes_hex": "74657874757265732f6578616d706c652e646473",
      "old": {
        "size": 7,
        "compressed_size": null,
        "payload_fingerprint": "cfb8a9d063b5e9e5"
      },
      "new": {
        "size": 8,
        "compressed_size": null,
        "payload_fingerprint": "50055d1b09262675"
      }
    }
  ],
  "unchanged": 1
}
]=]

local SizedDiffJson = [=[
{
  "old": "g.ba2",
  "new": "gc.ba2",
  "comparison": "metadata-only",
  "fingerprint_payloads": false,
  "added": [],
  "removed": [],
  "changed": [
    {
      "path": "readme.txt",
      "path_bytes_hex": "726561646d652e747874",
      "old": {
        "size": 6,
        "compressed_size": null
      },
      "new": {
        "size": 6,
        "compressed_size": 14
      }
    }
  ],
  "unchanged": 2
}
]=]

local ExtractAllPlanJson = [=[
{
  "operation": "extract-all",
  "archive": "<base>/t3.bsa",
  "output": "<base>/out",
  "entries": [
    {
      "action": "<action>",
      "path": "textures/example.dds",
      "path_bytes_hex": "74657874757265732f6578616d706c652e646473",
      "target": "<base>/out/textures/example.dds"
    },
    {
      "action": "<action>",
      "path": "meshes/example.nif",
      "path_bytes_hex": "6d65736865732f6578616d706c652e6e6966",
      "target": "<base>/out/meshes/example.nif"
    },
    {
      "action": "<action>",
      "path": "readme.txt",
      "path_bytes_hex": "726561646d652e747874",
      "target": "<base>/out/readme.txt"
    }
  ]
}
]=]

local CreatePlanJson = [=[
{
  "operation": "create",
  "format": "<format>",
  "output": "<base>/dry.archive",
  "files": 3,
  "entries": [
    {
      "action": "add",
      "source": "<base>/in/meshes/Example.NIF",
      "path": "meshes/example.nif",
      "path_bytes_hex": "6d65736865732f6578616d706c652e6e6966",
      "size": 39
    },
    {
      "action": "add",
      "source": "<base>/in/readme.txt",
      "path": "readme.txt",
      "path_bytes_hex": "726561646d652e747874",
      "size": 6
    },
    {
      "action": "add",
      "source": "<base>/in/textures/example.dds",
      "path": "textures/example.dds",
      "path_bytes_hex": "74657874757265732f6578616d706c652e646473",
      "size": 7
    }
  ]
}
]=]

local AddPlanJson = [=[
{
  "operation": "add",
  "archive": "<base>/t3.bsa",
  "output": "<output>",
  "format": "tes3",
  "files": 4,
  "added": 1,
  "replaced": 2,
  "preserved": 1,
  "entries": [
    {
      "action": "preserve",
      "path": "meshes/example.nif",
      "path_bytes_hex": "6d65736865732f6578616d706c652e6e6966"
    },
    {
      "action": "add",
      "source": "<base>/in2/added.txt",
      "path": "added.txt",
      "path_bytes_hex": "61646465642e747874",
      "size": 5
    },
    {
      "action": "replace",
      "source": "<base>/in2/readme.txt",
      "path": "readme.txt",
      "path_bytes_hex": "726561646d652e747874",
      "size": 6
    },
    {
      "action": "replace",
      "source": "<base>/in2/textures/example.dds",
      "path": "textures/example.dds",
      "path_bytes_hex": "74657874757265732f6578616d706c652e646473",
      "size": 8
    }
  ]
}
]=]

local VerifyJson = [=[
{
  "path": "<base>/t3.bsa",
  "format": "tes3",
  "file_count": 3,
  "named_entry_count": 3,
  "unnameable_entries": 0,
  "rewritable": true,
  "duplicate_normalized_paths": [],
  "unsafe_paths": [],
  "payloads_read": <read>,
  "warnings": []
}
]=]

--- `template` with each `<name>` in it replaced by what `values` has for the name.
local function filled(template: string, values: { [string]: string }): string
  return (
    string.gsub(
      template,
      '<(%l+)>',
      function(name: string): string return values[name] or `<{name}>` end
    )
  )
end

local counter = 0

--- A new folder for one case.
local function folder(): string
  local root = assert(rawget(_G, 'scratchRoot') :: string?, 'the case needs a scratchRoot')
  counter += 1
  local path = `{root}/cli-{counter}`
  assert(fs.mkdir(path, { recursive = true }))
  return path
end

local function write(path: string, data: string)
  local parent = string.match(path, '^(.*)/[^/]*$') :: string
  assert(fs.mkdir(parent, { recursive = true }))
  assert(fs.writeFile(path, data))
end

local function read(path: string): string?
  return (fs.readFileString(path))
end

--- A file's length and FNV-1a fingerprint, to compare an archive written here with the one the
--- original wrote from the same inputs.
local function digest(path: string): string
  local data = assert(fs.readFileString(path))
  local fingerprint: any = bytes.fnv1a64(data)
  return string.format('%d %016x', #data, fingerprint)
end

local function done(stdout: string): cli.Output
  return { status = 0, stdout = stdout, stderr = '' }
end

local function failed(message: string): cli.Output
  return { status = 9, stdout = '', stderr = `ERROR: {message}\n` }
end

local function unusable(message: string): cli.Output
  return { status = 8, stdout = '', stderr = `dream-archivetool: {message}\n` }
end

--- The two input folders every comparison with the original packed, in a new folder, and the
--- TES3 archive of the first as the command line creates it.
local function fixture(): string
  local base = folder()
  write(`{base}/in/textures/example.dds`, 'payload')
  write(`{base}/in/meshes/Example.NIF`, Mesh)
  write(`{base}/in/readme.txt`, 'readme')
  write(`{base}/in2/textures/example.dds`, 'PAYLOAD2')
  write(`{base}/in2/added.txt`, 'added')
  write(`{base}/in2/readme.txt`, 'readme')
  local created = cli.execute { 'create', `{base}/t3.bsa`, `{base}/in`, '--format', 'tes3' }
  assert(created.status == 0, created.stderr)
  return base
end

suite.describe('parse', function()
  suite.it('reads each subcommand\'s arguments and flags onto its request', function()
    for _, case in Read do
      local command, message = cli.parse(case.argv)
      expect(message).toBeNil()
      expect(command).toEqual(case.command)
    end
  end)

  suite.it('refuses what it can\'t read, and says why', function()
    for _, case in Refused do
      local command, message = cli.parse(case.argv)
      expect(command).toBeNil()
      expect(message).toBe(case.message)
    end
  end)

  suite.it('gives each add its own list of inputs, and leaves the argument list as it was', function()
    local argv = { 'add', 'a.bsa', 'one', '--', '-two' }
    local first, second = assert(cli.parse(argv)), assert(cli.parse(argv))
    expect(argv).toEqual { 'add', 'a.bsa', 'one', '--', '-two' }
    assert(first.action == 'add' and second.action == 'add')
    expect(first.request.inputs).toEqual { 'one', '-two' }
    expect(first.request.inputs ~= second.request.inputs).toBe(true)
  end)
end)

suite.describe('render', function()
  suite.it('prints info as two lines, or as the report', function()
    local result: commands.InfoResult = {
      command = 'info',
      info = {
        path = 't3.bsa',
        format = 'bsaTes3',
        fileCount = 3,
        namedEntryCount = 3,
        hasUnnameableEntries = false,
        rewritable = true,
      },
    }
    expect(cli.render(result)).toBe 'format: tes3\nfiles: 3\n'
    expect(cli.render(result, {})).toBe 'format: tes3\nfiles: 3\n'
    expect(cli.render(result, { json = true })).toBe(InfoJson)
  end)

  suite.it('spells a TES4 archive\'s flags and name mode, and a BA2\'s facts, as serde did', function()
    local compressed: commands.InfoResult = {
      command = 'info',
      info = {
        path = 't4c.bsa',
        format = 'bsaTes4',
        fileCount = 3,
        namedEntryCount = 3,
        hasUnnameableEntries = false,
        rewritable = true,
        tes4 = {
          version = 'v105',
          archiveTypes = 'ArchiveTypes(256)',
          archiveTypesBits = 256,
          archiveFlags = { 'directoryStrings', 'fileStrings', 'compressed' },
          archiveFlagsBits = 7,
          unsupportedArchiveFlagsBits = 0,
          nameMode = 'strings',
        },
      },
    }
    expect(cli.render(compressed, { json = true })).toBe(Tes4InfoJson)
    expect(cli.render(compressed)).toBe 'format: tes4\nfiles: 3\n'

    local hashed: commands.InfoResult = {
      command = 'info',
      info = {
        path = 'hash.bsa',
        format = 'bsaTes4',
        fileCount = 1,
        namedEntryCount = 0,
        hasUnnameableEntries = true,
        rewritable = false,
        rewriteBlocker = 'archive contains entries without recoverable paths; refusing to rewrite it lossy',
        tes4 = {
          version = 'v104',
          archiveTypes = 'ArchiveTypes(256)',
          archiveTypesBits = 256,
          archiveFlags = {},
          archiveFlagsBits = 0,
          unsupportedArchiveFlagsBits = 0,
          nameMode = 'hashOnly',
        },
      },
    }
    expect(cli.render(hashed, { json = true })).toBe(HashOnlyInfoJson)

    local general: commands.InfoResult = {
      command = 'info',
      info = {
        path = 'gc.ba2',
        format = 'ba2',
        fileCount = 3,
        namedEntryCount = 3,
        hasUnnameableEntries = false,
        rewritable = true,
        ba2 = { version = 'v2', payloadFormat = 'gnrl', compressionFormat = 'zip', strings = true },
      },
    }
    expect(cli.render(general, { json = true })).toBe(Ba2InfoJson)
    expect(cli.render(general)).toBe 'format: ba2\nfiles: 3\n'
  end)

  suite.it('prints a listing as paths, with sizes, or as entries with a null for no size', function()
    local result: commands.ListResult = {
      command = 'list',
      archive = 'mixed.archive',
      entries = {
        { path = 'readme.txt', pathBytesHex = ReadmeHex, size = 6, compressedSize = 14 },
        { path = 'meshes/example.nif', pathBytesHex = MeshHex, size = 39 },
        { path = 'textures/example.dds', pathBytesHex = ExampleHex },
      },
    }
    expect(cli.render(result)).toBe 'readme.txt\nmeshes/example.nif\ntextures/example.dds\n'
    expect(cli.render(result, { long = true })).toBe '         6 readme.txt\n        39 meshes/example.nif\n         - textures/example.dds\n'
    expect(cli.render(result, { json = true })).toBe(ListJson)

    local empty: commands.ListResult = { command = 'list', archive = 'hash.bsa', entries = {} }
    expect(cli.render(empty)).toBe ''
    expect(cli.render(empty, { long = true })).toBe ''
    expect(cli.render(empty, { json = true })).toBe '[]\n'
  end)

  suite.it('escapes a name as serde_json does, and leaves what isn\'t ASCII as it is', function()
    local result: commands.ListResult = {
      command = 'list',
      archive = 'odd.bsa',
      entries = {
        {
          path = 'textures/invalid-\239\191\189.dds',
          pathBytesHex = '74657874757265732f696e76616c69642dff2e646473',
          size = 5,
        },
        {
          path = 'quote"back/slash/tab\there.txt',
          pathBytesHex = '71756f7465226261636b2f736c6173682f74616209686572652e747874',
          size = 7,
        },
      },
    }
    expect(cli.render(result, { json = true })).toBe(OddListJson)
    expect(cli.render(result, { long = true })).toBe '         5 textures/invalid-\239\191\189.dds\n         7 quote"back/slash/tab\there.txt\n'

    local control: commands.ListResult = {
      command = 'list',
      archive = 'control.bsa',
      entries = { { path = 'a\\b\1\n\r\8\12\127.txt', pathBytesHex = '00' } },
    }
    expect(cli.render(control, { json = true })).toContain '"path": "a\\\\b\\u0001\\n\\r\\b\\f\127.txt",'
  end)

  suite.it('escapes each control character, the quote, and the backslash, and no other byte', function()
    local short: { [number]: string } = {
      [8] = '\\b',
      [9] = '\\t',
      [10] = '\\n',
      [12] = '\\f',
      [13] = '\\r',
      [34] = '\\"',
      [92] = '\\\\',
    }

    for code = 0, 255 do
      local plain = if code < 32 then string.format('\\u%04x', code) else string.char(code)
      local result: commands.ListResult = {
        command = 'list',
        archive = 'bytes.bsa',
        entries = { { path = `a{string.char(code)}z{string.char(code)}`, pathBytesHex = '00' } },
      }
      local written = short[code] or plain
      expect(cli.render(result, { json = true })).toContain(`"path": "a{written}z{written}",`)
    end
  end)

  suite.it('prints a verification with its blocker, payloads, and warnings', function()
    local hashed: commands.VerifyResult = {
      command = 'verify',
      report = {
        path = 'hash.bsa',
        format = 'bsaTes4',
        fileCount = 1,
        namedEntryCount = 0,
        unnameableEntries = 1,
        rewritable = false,
        rewriteBlocker = 'archive contains entries without recoverable paths; refusing to rewrite it lossy',
        duplicateNormalizedPaths = {},
        unsafePaths = {},
        payloadsRead = 0,
        warnings = { 'archive contains entries without recoverable path names' },
      },
    }
    expect(cli.render(hashed)).toBe(HashOnlyVerify)

    local plain: commands.VerifyResult = {
      command = 'verify',
      report = {
        path = 't3.bsa',
        format = 'bsaTes3',
        fileCount = 3,
        namedEntryCount = 3,
        unnameableEntries = 0,
        rewritable = true,
        duplicateNormalizedPaths = {},
        unsafePaths = {},
        warnings = {},
      },
    }
    expect(cli.render(plain)).toBe 'format: tes3\nfiles: 3\nnamed: 3\nunnameable: 0\nrewritable: true\n'
  end)

  suite.it('prints a verification\'s path issues with the stored bytes they have', function()
    local duplicate: commands.VerifyResult = {
      command = 'verify',
      report = {
        path = 'dup.bsa',
        format = 'bsaTes3',
        fileCount = 2,
        namedEntryCount = 2,
        unnameableEntries = 0,
        rewritable = true,
        duplicateNormalizedPaths = {
          {
            path = 'textures/example.dds',
            pathBytesHex = ExampleHex,
            rawPathBytesHex = ExampleHex,
            collidingRawPathBytesHex = '54657874757265732f4578616d706c652e646473',
          },
        },
        unsafePaths = {},
        warnings = {
          'archive contains duplicate normalized paths',
          'payload read verification skipped because duplicate normalized paths prevent per-entry coverage',
        },
      },
    }
    expect(cli.render(duplicate, { json = true })).toBe(DuplicateVerifyJson)

    local unsafe: commands.VerifyResult = {
      command = 'verify',
      report = {
        path = 'unsafe.bsa',
        format = 'bsaTes3',
        fileCount = 2,
        namedEntryCount = 2,
        unnameableEntries = 0,
        rewritable = true,
        duplicateNormalizedPaths = {},
        unsafePaths = {
          {
            path = '../../c.dds',
            pathBytesHex = '2e2e2f2e2e2f632e646473',
            rawPathBytesHex = '2e2e5c2e2e5c432e646473',
          },
        },
        payloadsRead = 2,
        warnings = { 'archive contains paths unsafe to extract directly' },
      },
    }
    expect(cli.render(unsafe, { json = true })).toBe(UnsafeVerifyJson)
  end)

  suite.it('prints a comparison as counts, or as the report with fingerprints in hexadecimal', function()
    local result: commands.DiffResult = {
      command = 'diff',
      report = {
        old = 't3.bsa',
        new = 't3b.bsa',
        comparison = 'payloadFingerprint',
        fingerprintPayloads = true,
        added = {
          {
            path = 'added.txt',
            pathBytesHex = '61646465642e747874',
            size = 5,
            payloadFingerprint = bytes.fnv1a64 'added',
          },
        },
        removed = {
          {
            path = 'meshes/example.nif',
            pathBytesHex = MeshHex,
            size = 39,
            payloadFingerprint = bytes.fnv1a64(Mesh),
          },
        },
        changed = {
          {
            path = 'textures/example.dds',
            pathBytesHex = ExampleHex,
            old = { size = 7, payloadFingerprint = bytes.fnv1a64 'payload' },
            new = { size = 8, payloadFingerprint = bytes.fnv1a64 'PAYLOAD2' },
          },
        },
        unchanged = 1,
      },
    }
    expect(cli.render(result)).toBe 'comparison: payload-fingerprint\nadded: 1\nremoved: 1\nchanged: 1\nunchanged: 1\n'
    expect(cli.render(result, { json = true })).toBe(DiffJson)

    local sized: commands.DiffResult = {
      command = 'diff',
      report = {
        old = 'g.ba2',
        new = 'gc.ba2',
        comparison = 'metadataOnly',
        fingerprintPayloads = false,
        added = {},
        removed = {},
        changed = {
          {
            path = 'readme.txt',
            pathBytesHex = ReadmeHex,
            old = { size = 6 },
            new = { size = 6, compressedSize = 14 },
          },
        },
        unchanged = 2,
      },
    }
    expect(cli.render(sized)).toBe 'comparison: metadata-only\nadded: 0\nremoved: 0\nchanged: 1\nunchanged: 2\n'
    expect(cli.render(sized, { json = true })).toBe(SizedDiffJson)
  end)

  suite.it('prints an extraction as its counts, its bytes, or nothing when it was refused', function()
    local row: commands.ExtractResult = {
      command = 'extract',
      dryRun = false,
      conflicts = {},
      summary = { extracted = 1, skipped = 0 },
      written = { 'out/readme.txt' },
    }
    expect(cli.render(row)).toBe 'extracted: 1\n'
    expect(cli.render(row, { json = true })).toBe '{\n  "extracted": 1,\n  "skipped": 0\n}\n'
    row.summary = { extracted = 0, skipped = 1 }
    expect(cli.render(row)).toBe 'extracted: 0\nskipped: 1\n'
    expect(cli.render(row, { json = true })).toBe '{\n  "extracted": 0,\n  "skipped": 1\n}\n'

    local bytesOnly: commands.ExtractResult = {
      command = 'extract',
      dryRun = false,
      conflicts = {},
      written = {},
      data = 'pay\0load\255',
    }
    expect(cli.render(bytesOnly)).toBe 'pay\0load\255'

    local refused: commands.ExtractAllResult = {
      command = 'extractAll',
      dryRun = false,
      plan = { operation = 'extractAll', archive = 't3.bsa', output = 'out', entries = {} },
      conflicts = {
        { action = 'conflict', path = 'readme.txt', pathBytesHex = ReadmeHex, target = 'out/readme.txt' },
      },
      written = {},
    }
    expect(cli.render(refused)).toBe ''
    expect(cli.render(refused, { json = true })).toBe ''
  end)

  suite.it('prints a dry run\'s plan as JSON whether or not it was asked for', function()
    local rows: { tool.ExtractPlanRow } = {
      {
        action = 'conflict',
        path = 'textures/example.dds',
        pathBytesHex = ExampleHex,
        target = '/scratch/out/textures/example.dds',
      },
      {
        action = 'conflict',
        path = 'meshes/example.nif',
        pathBytesHex = MeshHex,
        target = '/scratch/out/meshes/example.nif',
      },
      {
        action = 'conflict',
        path = 'readme.txt',
        pathBytesHex = ReadmeHex,
        target = '/scratch/out/readme.txt',
      },
    }

    local planned: commands.ExtractAllResult = {
      command = 'extractAll',
      dryRun = true,
      plan = {
        operation = 'extractAll',
        archive = '/scratch/t3.bsa',
        output = '/scratch/out',
        entries = rows,
      },
      conflicts = rows,
      written = {},
    }
    local expected = filled(ExtractAllPlanJson, { base = '/scratch', action = 'conflict' })
    expect(cli.render(planned)).toBe(expected)
    expect(cli.render(planned, { json = true })).toBe(expected)

    local one: commands.ExtractResult = {
      command = 'extract',
      dryRun = true,
      plan = { operation = 'extract', archive = 't3.bsa', output = '.', entries = {} },
      conflicts = {},
      written = {},
    }
    expect(cli.render(one)).toBe '{\n  "operation": "extract",\n  "archive": "t3.bsa",\n  "output": ".",\n  "entries": []\n}\n'
  end)

  suite.it('prints a written archive as its count, and a planned one as its plan', function()
    local plan = {
      operation = 'create' :: 'create',
      format = 'ba2' :: 'ba2',
      output = '/scratch/dry.archive',
      files = 3,
      entries = {
        {
          action = 'add' :: 'add',
          source = '/scratch/in/meshes/Example.NIF',
          path = 'meshes/example.nif',
          pathBytesHex = MeshHex,
          size = 39,
        },
        {
          action = 'add' :: 'add',
          source = '/scratch/in/readme.txt',
          path = 'readme.txt',
          pathBytesHex = ReadmeHex,
          size = 6,
        },
        {
          action = 'add' :: 'add',
          source = '/scratch/in/textures/example.dds',
          path = 'textures/example.dds',
          pathBytesHex = ExampleHex,
          size = 7,
        },
      },
    }

    local written: commands.CreateResult = {
      command = 'create',
      dryRun = false,
      plan = plan,
      files = 3,
      written = { '/scratch/dry.archive' },
    }
    expect(cli.render(written)).toBe 'files: 3\n'
    expect(cli.render(written, { json = true })).toBe '{\n  "files": 3\n}\n'

    local planned: commands.CreateResult =
      { command = 'create', dryRun = true, plan = plan, written = {} }
    local expected = filled(CreatePlanJson, { base = '/scratch', format = 'ba2' })
    expect(cli.render(planned)).toBe(expected)
    expect(cli.render(planned, { json = true })).toBe(expected)
  end)

  suite.it('prints an addition as its count, and a planned one with what it keeps', function()
    local plan = {
      operation = 'add' :: 'add',
      archive = '/scratch/t3.bsa',
      output = '/scratch/t3.bsa',
      format = 'bsaTes3' :: 'bsaTes3',
      files = 4,
      added = 1,
      replaced = 2,
      preserved = 1,
      entries = {
        { action = 'preserve' :: 'preserve', path = 'meshes/example.nif', pathBytesHex = MeshHex },
        {
          action = 'add' :: 'add',
          source = '/scratch/in2/added.txt',
          path = 'added.txt',
          pathBytesHex = '61646465642e747874',
          size = 5,
        },
        {
          action = 'replace' :: 'replace',
          source = '/scratch/in2/readme.txt',
          path = 'readme.txt',
          pathBytesHex = ReadmeHex,
          size = 6,
        },
        {
          action = 'replace' :: 'replace',
          source = '/scratch/in2/textures/example.dds',
          path = 'textures/example.dds',
          pathBytesHex = ExampleHex,
          size = 8,
        },
      },
    }
    local planned: commands.AddResult = { command = 'add', dryRun = true, plan = plan, written = {} }
    expect(cli.render(planned)).toBe(
      filled(AddPlanJson, { base = '/scratch', output = '/scratch/t3.bsa' })
    )

    local written: commands.AddResult = {
      command = 'add',
      dryRun = false,
      plan = plan,
      files = 4,
      written = { '/scratch/t3.bsa' },
    }
    expect(cli.render(written)).toBe 'files: 4\n'
    expect(cli.render(written, { json = true })).toBe '{\n  "files": 4\n}\n'
  end)
end)

suite.describe('status and diagnostic', function()
  local conflict = {
    action = 'conflict' :: 'conflict',
    path = 'readme.txt',
    pathBytesHex = ReadmeHex,
    target = 'out/readme.txt',
  }

  local later = {
    action = 'conflict' :: 'conflict',
    path = 'zebra.txt',
    pathBytesHex = '7a656272612e747874',
    target = 'out/zebra.txt',
  }

  suite.it('number the exit statuses as the other targets do', function()
    expect(cli.Exit).toEqual { ok = 0, invalidInput = 8, runtime = 9 }
  end)

  suite.it('end an extraction that stopped at a target with 9 and the first one it met', function()
    local one: commands.ExtractResult = {
      command = 'extract',
      dryRun = false,
      plan = { operation = 'extract', archive = 't3.bsa', output = 'out', entries = { conflict } },
      conflicts = { conflict },
      written = {},
    }
    expect(cli.status(one)).toBe(9)
    expect(cli.diagnostic(one)).toBe 'target already exists: out/readme.txt'

    local every: commands.ExtractAllResult = {
      command = 'extractAll',
      dryRun = false,
      plan = { operation = 'extractAll', archive = 't3.bsa', output = 'out', entries = { conflict, later } },
      conflicts = { conflict, later },
      written = {},
    }
    expect(cli.status(every)).toBe(9)
    expect(cli.diagnostic(every)).toBe 'target already exists: out/readme.txt'

    -- A dry run only reports the targets that are there.
    every.dryRun = true
    expect(cli.status(every)).toBe(0)
    expect(cli.diagnostic(every)).toBeNil()
  end)

  suite.it('end every other result with 0 and nothing to say', function()
    local results: { commands.Result } = {
      {
        command = 'info',
        info = {
          path = 't3.bsa',
          format = 'bsaTes3',
          fileCount = 3,
          namedEntryCount = 3,
          hasUnnameableEntries = false,
          rewritable = true,
        },
      },
      { command = 'list', archive = 't3.bsa', entries = {} },
      {
        command = 'extract',
        dryRun = false,
        conflicts = {},
        summary = { extracted = 0, skipped = 1 },
        written = {},
      },
      { command = 'extract', dryRun = false, conflicts = {}, written = {}, data = '' },
    }

    for _, result in results do
      expect(cli.status(result)).toBe(0)
      expect(cli.diagnostic(result)).toBeNil()
    end
  end)
end)

suite.describe('errorMessage', function()
  suite.it('names the flags where the library names its own options', function()
    expect(cli.errorMessage 'compress is not valid with format bsaTes3').toBe 'archive error: --compress is not valid with --format tes3'
    expect(cli.errorMessage 'tes4Version is not valid with format ba2').toBe 'archive error: --tes4-version is not valid with --format ba2'
    expect(cli.errorMessage 'ba2Kind is not valid with format bsaTes4').toBe 'archive error: --ba2-kind is not valid with --format tes4'
    expect(cli.errorMessage 'ba2Version is not valid with format bsaTes3').toBe 'archive error: --ba2-version is not valid with --format tes3'
    expect(
      cli.errorMessage 'refusing to follow symlink input path: in/link.txt; set followSymlinks to opt in'
    ).toBe 'refusing to follow symlink input path: in/link.txt; pass --follow-symlinks to opt in'
    expect(
      cli.errorMessage 'planExtractByPathHex.entries[1]: invalid pathBytesHex: archive error: archive path hex must contain an even number of hexadecimal digits'
    ).toBe 'archive error: --entry-hex must contain an even number of hexadecimal digits'
    expect(
      cli.errorMessage 'readEntryByPathHex: invalid pathBytesHex: archive error: archive path hex contains a non-hexadecimal digit'
    ).toBe 'archive error: --entry-hex contains a non-hexadecimal digit'
  end)

  suite.it('leaves every other message as the library words it', function()
    for _, message in
      {
        'archive entry not found: missing.dds',
        'unsafe archive path: ../evil',
        'target already exists: out/readme.txt',
        'I/O error: No such file or directory (os error 2)',
        'archive error: failed to open archive \'missing.bsa\': No such file or directory (os error 2)',
        'archive error: DX10 BA2 archives can only contain .dds files: meshes/example.nif',
      }

    do
      expect(cli.errorMessage(message)).toBe(message)
    end
  end)
end)

suite.describe('execute', function()
  suite.it('prints the help, a subcommand\'s help, and the version', function()
    local help = cli.execute {}
    expect(help.status).toBe(0)
    expect(help.stderr).toBe ''
    expect(help.stdout).toContain 'dream_archivetool 1.1.0 for Jess, run here as dream-archivetool: inspect and manipulate Bethesda\nBSA and BA2 archives\n\nUsage: dream-archivetool [OPTIONS] [COMMAND]\n'
    expect(help.stdout).toContain 'Exit status: 0 done, 8 the command line can\'t be used, 9 the run failed; the original exits 2\nand 1 for those.'
    expect(help.stdout).toContain 'Not part of this port, and refused by name: --generate-completion and --generate-manpage'
    expect(cli.execute { '--help' }).toEqual(help)
    expect(cli.execute { 'help' }).toEqual(help)

    for _, name in { 'info', 'list', 'verify', 'diff', 'extract', 'extract-all', 'create', 'add' } do
      local topic = cli.execute { 'help', name }
      expect(topic.status).toBe(0)
      expect(topic.stdout).toContain(`\n\nUsage: dream-archivetool {name} [OPTIONS] `)
      expect(topic.stdout).toContain '\n  -h, --help'
      expect(string.sub(topic.stdout, -#Statuses)).toBe(Statuses)
      expect(cli.execute { name, '--help' }).toEqual(topic)
      expect(help.stdout).toContain(`\n  {name} `)
    end

    expect(cli.execute({ 'help', 'info' }).stdout).toBe 'Print archive metadata\n\nUsage: dream-archivetool info [OPTIONS] <ARCHIVE>\n\nArguments:\n  <ARCHIVE>  Archive path\n\nOptions:\n      --json  Write JSON to stdout\n  -h, --help  Print help\n\nExit status: 0 done, 8 the command line can\'t be used, 9 the run failed.\n'
    expect(cli.execute { '--version' }).toEqual(done 'dream-archivetool 1.1.0\n')
    expect(cli.execute { '-V' }).toEqual(done 'dream-archivetool 1.1.0\n')
  end)

  suite.it('refuses every command line parse refuses, with 8 and the reason', function()
    for _, case in Refused do
      expect(cli.execute(case.argv)).toEqual(unusable(case.message))
    end
  end)

  suite.it('creates the archives the original creates from the same files', function()
    local base = fixture()
    local input = `{base}/in`
    expect(digest(`{base}/t3.bsa`)).toBe '175 347e79bf51bcdbc2'

    -- Each argument list after the archive and the input, and the original's archive for it.
    local cases: { { name: string, flags: { string }, stdout: string, digest: string } } = {
      { name = 't4.bsa', flags = { '--format', 'tes4' }, stdout = 'files: 3\n', digest = '239 412d7199bf53f231' },
      {
        name = 't4s.bsa',
        flags = { '--format', 'tes4', '--tes4-version', 'skyrim' },
        stdout = 'files: 3\n',
        digest = '239 6d95accedc0256d0',
      },
      {
        name = 't4f.bsa',
        flags = { '--format', 'tes4', '--tes4-version', 'fallout3', '--json' },
        stdout = '{\n  "files": 3\n}\n',
        digest = '239 6d95accedc0256d0',
      },
      {
        name = 't4c.bsa',
        flags = { '--format', 'tes4', '--compress', '--tes4-version', 'skyrim-se' },
        stdout = 'files: 3\n',
        digest = '297 ed5126f83b7768eb',
      },
      {
        name = 't4oc.bsa',
        flags = { '--format', 'tes4', '--compress' },
        stdout = 'files: 3\n',
        digest = '244 78b6fbda4a047bee',
      },
      { name = 'g.ba2', flags = { '--format', 'ba2' }, stdout = 'files: 3\n', digest = '238 8114b4737e3d5edb' },
      {
        name = 'gc.ba2',
        flags = { '--format', 'ba2', '--compress', '--ba2-version', 'starfield', '--json' },
        stdout = '{\n  "files": 3\n}\n',
        digest = '239 3e5d976f5763c50d',
      },
      {
        name = 'gn.ba2',
        flags = { '--format', 'ba2', '--ba2-version', 'fallout4-next-gen', '--ba2-kind', 'gnrl' },
        stdout = 'files: 3\n',
        digest = '238 db5a4da3d5172846',
      },
    }

    for _, case in cases do
      local argv = { 'create', `{base}/{case.name}`, input }
      table.move(case.flags, 1, #case.flags, 4, argv)
      expect(cli.execute(argv)).toEqual(done(case.stdout))
      expect(digest(`{base}/{case.name}`)).toBe(case.digest)
    end

    write(`{base}/single.txt`, 'lonely')
    expect(cli.execute { 'create', `{base}/one.bsa`, `{base}/single.txt`, '--format', 'tes3' }).toEqual(
      done 'files: 1\n'
    )
    expect(digest(`{base}/one.bsa`)).toBe '49 e66f8739135c1b2d'
  end)

  suite.it('plans a creation as JSON without writing, with or without --json', function()
    local base = fixture()
    local output = `{base}/dry.archive`
    local tes3 = done(filled(CreatePlanJson, { base = base, format = 'tes3' }))
    expect(cli.execute { 'create', output, `{base}/in`, '--format', 'tes3', '--dry-run' }).toEqual(tes3)
    expect(cli.execute { 'create', output, `{base}/in`, '--format', 'tes3', '--dry-run', '--json' }).toEqual(
      tes3
    )
    expect(cli.execute { 'create', output, `{base}/in`, '--format', 'ba2', '--dry-run' }).toEqual(
      done(filled(CreatePlanJson, { base = base, format = 'ba2' }))
    )
    expect((fs.exists(output))).toBe(false)
  end)

  suite.it('prints an archive\'s facts, entries, and verification as the original does', function()
    local base = fixture()
    local path = `{base}/t3.bsa`
    expect(cli.execute { 'info', path }).toEqual(done 'format: tes3\nfiles: 3\n')
    expect(cli.execute { 'info', path, '--json' }).toEqual(
      done((string.gsub(InfoJson, '"t3.bsa"', `"{path}"`)))
    )
    expect(cli.execute { 'list', path }).toEqual(
      done 'textures/example.dds\nmeshes/example.nif\nreadme.txt\n'
    )
    expect(cli.execute { 'list', '-l', path }).toEqual(
      done '         7 textures/example.dds\n        39 meshes/example.nif\n         6 readme.txt\n'
    )
    expect(cli.execute { 'list', '--json', path }).toEqual(
      done(
        `[\n  \{\n    "path": "textures/example.dds",\n    "path_bytes_hex": "{ExampleHex}",\n    "size": 7,\n    "compressed_size": null\n  },\n  \{\n    "path": "meshes/example.nif",\n    "path_bytes_hex": "{MeshHex}",\n    "size": 39,\n    "compressed_size": null\n  },\n  \{\n    "path": "readme.txt",\n    "path_bytes_hex": "{ReadmeHex}",\n    "size": 6,\n    "compressed_size": null\n  }\n]\n`
      )
    )
    expect(cli.execute { 'verify', path }).toEqual(
      done 'format: tes3\nfiles: 3\nnamed: 3\nunnameable: 0\nrewritable: true\n'
    )
    expect(cli.execute { 'verify', path, '--read-payloads' }).toEqual(
      done 'format: tes3\nfiles: 3\nnamed: 3\nunnameable: 0\nrewritable: true\npayloads read: 3\n'
    )
    expect(cli.execute { 'verify', path, '--json' }).toEqual(
      done(filled(VerifyJson, { base = base, read = 'null' }))
    )
    expect(cli.execute { 'verify', path, '--read-payloads', '--json' }).toEqual(
      done(filled(VerifyJson, { base = base, read = '3' }))
    )
  end)

  suite.it('lists a TES4 archive\'s sizes, as dream_archivetool 1.1.0 does', function()
    local base = fixture()
    local path = `{base}/t4c.bsa`
    local created = cli.execute { 'create', path, `{base}/in`, '--format', 'tes4', '--compress' }
    expect(created.status).toBe(0)
    -- The installed 0.1.0 prints `-` for these; 1.1.0 reads each size from the index or the data.
    expect(cli.execute { 'list', '--long', path }).toEqual(
      done '         6 readme.txt\n        39 meshes/example.nif\n         7 textures/example.dds\n'
    )
    expect(cli.execute({ 'info', path, '--json' }).stdout).toContain '"archive_flags": [\n      "directory-strings",\n      "file-strings",\n      "compressed"\n    ],'
  end)

  suite.it('compares two archives as the original does', function()
    local base = fixture()
    local old, new = `{base}/t3.bsa`, `{base}/t3b.bsa`
    expect(cli.execute { 'create', new, `{base}/in2`, '--format', 'tes3' }).toEqual(done 'files: 3\n')
    expect(digest(new)).toBe '133 21209e135c8dbdc1'
    expect(cli.execute { 'diff', old, new }).toEqual(
      done 'comparison: metadata-only\nadded: 1\nremoved: 1\nchanged: 1\nunchanged: 1\n'
    )
    expect(cli.execute { 'diff', old, new, '--hash' }).toEqual(
      done 'comparison: payload-fingerprint\nadded: 1\nremoved: 1\nchanged: 1\nunchanged: 1\n'
    )
    local expected = string.gsub(DiffJson, '"t3b.bsa"', `"{new}"`)
    expected = string.gsub(expected, '"t3.bsa"', `"{old}"`)
    expect(cli.execute { 'diff', old, new, '--hash', '--json' }).toEqual(done(expected))
  end)

  suite.it('extracts one entry to standard output or to disk, under each overwrite policy', function()
    local base = fixture()
    local path, out = `{base}/t3.bsa`, `{base}/out`
    local target = `{out}/textures/example.dds`
    expect(cli.execute { 'extract', path, 'textures/example.dds', '--stdout' }).toEqual(done 'payload')
    expect(cli.execute { 'extract', path, '--entry-hex', ReadmeHex, '--stdout' }).toEqual(done 'readme')
    expect((fs.exists(out))).toBe(false)

    expect(cli.execute { 'extract', path, 'Textures\\Example.DDS', '-o', out }).toEqual(
      done 'extracted: 1\n'
    )
    expect(read(target)).toBe 'payload'
    assert(fs.writeFile(target, 'existing'))
    expect(cli.execute { 'extract', path, 'textures/example.dds', '-o', out }).toEqual(
      failed(`target already exists: {target}`)
    )
    expect(cli.execute { 'extract', path, 'textures/example.dds', '-o', out, '--json' }).toEqual(
      failed(`target already exists: {target}`)
    )
    expect(cli.execute { 'extract', path, 'textures/example.dds', '-o', out, '--skip-existing' }).toEqual(
      done 'extracted: 0\nskipped: 1\n'
    )
    expect(read(target)).toBe 'existing'
    expect(
      cli.execute { 'extract', path, 'textures/example.dds', '-o', out, '--skip-existing', '--json' }
    ).toEqual(done '{\n  "extracted": 0,\n  "skipped": 1\n}\n')
    expect(
      cli.execute { 'extract', path, 'textures/example.dds', '--output', out, '--overwrite', '--json' }
    ).toEqual(done '{\n  "extracted": 1,\n  "skipped": 0\n}\n')
    expect(read(target)).toBe 'payload'
    expect(cli.execute { 'extract', path, '--entry-hex', MeshHex, `--output={out}`, '--flat' }).toEqual(
      done 'extracted: 1\n'
    )
    expect(read(`{out}/example.nif`)).toBe(Mesh)
    expect((fs.exists(`{out}/meshes/example.nif`))).toBe(false)
  end)

  suite.it('reads an entry whose name isn\'t UTF-8 by its bytes or its hex', function()
    local path = `{folder()}/odd.bsa`
    local builder = archive.bsa.tes3.Builder.new()
    builder:addBytes('textures/invalid-\255.dds', 'bytes')
    builder:writePath(path)
    expect(cli.execute { 'extract', path, 'textures/invalid-\255.dds', '--stdout' }).toEqual(done 'bytes')
    expect(
      cli.execute { 'extract', path, '--entry-hex', '74657874757265732f696e76616c69642dff2e646473', '--stdout' }
    ).toEqual(done 'bytes')
    expect(cli.execute { 'list', path }).toEqual(done 'textures/invalid-\239\191\189.dds\n')
  end)

  suite.it('extracts every entry, planning as JSON and stopping at a target that is there', function()
    local base = fixture()
    local path, out = `{base}/t3.bsa`, `{base}/out`
    local plan = filled(ExtractAllPlanJson, { base = base, action = 'extract' })
    expect(cli.execute { 'extract-all', path, '-o', out, '--dry-run' }).toEqual(done(plan))
    expect(cli.execute { 'extract-all', path, '-o', out, '--dry-run', '--json' }).toEqual(done(plan))
    expect((fs.exists(out))).toBe(false)

    expect(cli.execute { 'extract-all', path, '-o', out }).toEqual(done 'extracted: 3\n')
    expect(read(`{out}/textures/example.dds`)).toBe 'payload'
    expect(read(`{out}/meshes/example.nif`)).toBe(Mesh)
    expect(read(`{out}/readme.txt`)).toBe 'readme'

    local stopped = failed(`target already exists: {out}/textures/example.dds`)
    expect(cli.execute { 'extract-all', path, '-o', out }).toEqual(stopped)
    expect(cli.execute { 'extract-all', path, '-o', out, '--json' }).toEqual(stopped)
    -- 1.1.0 plans over targets that are there; the installed 0.1.0 fails the plan instead.
    expect(cli.execute { 'extract-all', path, '-o', out, '--dry-run' }).toEqual(
      done(filled(ExtractAllPlanJson, { base = base, action = 'conflict' }))
    )
    expect(cli.execute { 'extract-all', path, '-o', out, '--dry-run', '--skip-existing' }).toEqual(
      done(filled(ExtractAllPlanJson, { base = base, action = 'skip' }))
    )
    expect(cli.execute { 'extract-all', path, '-o', out, '--dry-run', '--overwrite', '--json' }).toEqual(
      done(filled(ExtractAllPlanJson, { base = base, action = 'overwrite' }))
    )
    expect(cli.execute { 'extract-all', path, '-o', out, '--skip-existing' }).toEqual(
      done 'extracted: 0\nskipped: 3\n'
    )
    expect(cli.execute { 'extract-all', path, '-o', out, '--skip-existing', '--json' }).toEqual(
      done '{\n  "extracted": 0,\n  "skipped": 3\n}\n'
    )
    expect(cli.execute { 'extract-all', path, '-o', out, '--overwrite', '--json' }).toEqual(
      done '{\n  "extracted": 3,\n  "skipped": 0\n}\n'
    )
  end)

  suite.it('adds to an archive, planning as JSON and writing what the original writes', function()
    local base = fixture()
    local path, inputs, output = `{base}/t3.bsa`, `{base}/in2`, `{base}/updated.bsa`
    local inPlace = done(filled(AddPlanJson, { base = base, output = path }))
    expect(cli.execute { 'add', path, inputs, '--dry-run' }).toEqual(inPlace)
    expect(cli.execute { 'add', path, inputs, '--dry-run', '--json' }).toEqual(inPlace)
    expect(cli.execute { 'add', path, inputs, '-o', output, '--dry-run' }).toEqual(
      done(filled(AddPlanJson, { base = base, output = output }))
    )
    expect((fs.exists(output))).toBe(false)
    expect(digest(path)).toBe '175 347e79bf51bcdbc2'

    expect(cli.execute { 'add', path, inputs, '-o', output }).toEqual(done 'files: 4\n')
    expect(digest(output)).toBe '211 0a59dd88023c388e'
    expect(digest(path)).toBe '175 347e79bf51bcdbc2'
    expect(cli.execute { 'add', path, inputs, '--json' }).toEqual(done '{\n  "files": 4\n}\n')
    expect(digest(path)).toBe '211 0a59dd88023c388e'
    expect(cli.execute { 'add', path, `{inputs}/added.txt`, `{inputs}/textures` }).toEqual(
      done 'files: 5\n'
    )
    expect(digest(path)).toBe '251 aba43607915698e1'
    expect(cli.execute { 'list', path }).toEqual(
      done 'textures/example.dds\nmeshes/example.nif\nreadme.txt\nadded.txt\nexample.dds\n'
    )
  end)

  suite.it('creates, plans, and adds to archives named beside their inputs', function()
    -- The original's case for bare names in the working folder, which a suite can't change:
    -- the same commands on paths below the scratch folder.
    local base = folder()
    write(`{base}/input.txt`, 'payload')
    write(`{base}/extra.txt`, 'payload')
    local path, second = `{base}/MyMod.bsa`, `{base}/MyMod-2.bsa`
    expect(cli.execute { 'create', path, `{base}/input.txt`, '--format', 'tes3' }).toEqual(
      done 'files: 1\n'
    )
    local plan = cli.execute { 'add', path, `{base}/extra.txt`, '--output', second, '--dry-run' }
    expect(plan.status).toBe(0)
    expect(plan.stdout).toContain(`\n  "output": "{second}",\n`)
    expect(cli.execute { 'add', path, `{base}/extra.txt`, '--output', second }).toEqual(
      done 'files: 2\n'
    )
    expect(cli.execute { 'add', path, `{base}/extra.txt` }).toEqual(done 'files: 2\n')
    expect((fs.exists(second))).toBe(true)
    expect(cli.execute { 'list', path }).toEqual(done 'extra.txt\ninput.txt\n')
  end)

  suite.it('ends a run that can\'t be carried out with 9 and the original\'s words', function()
    local base = fixture()
    local path, missing = `{base}/t3.bsa`, `{base}/missing.bsa`
    local unopened = failed(
      `archive error: failed to open archive '{missing}': No such file or directory (os error 2)`
    )
    expect(cli.execute { 'info', missing }).toEqual(unopened)
    expect(cli.execute { 'list', missing }).toEqual(unopened)
    expect(cli.execute { 'verify', missing, '--json' }).toEqual(unopened)
    expect(cli.execute { 'diff', path, missing }).toEqual(unopened)
    expect(cli.execute { 'extract', missing, 'a.dds', '--stdout' }).toEqual(unopened)
    expect(cli.execute { 'extract-all', missing, '--dry-run' }).toEqual(unopened)
    expect(cli.execute { 'add', missing, `{base}/in2` }).toEqual(unopened)
    expect(cli.execute { 'list', `{base}/in/readme.txt` }).toEqual(
      failed(
        `archive error: failed to open archive '{base}/in/readme.txt': unknown or disabled archive format`
      )
    )

    expect(cli.execute { 'extract', path, 'missing.dds', '-o', `{base}/out` }).toEqual(
      failed 'archive entry not found: missing.dds'
    )
    expect(cli.execute { 'extract', path, 'missing.dds', '--stdout' }).toEqual(
      failed 'archive entry not found: missing.dds'
    )
    expect(cli.execute { 'extract', path, '../evil', '-o', `{base}/out` }).toEqual(
      failed 'unsafe archive path: ../evil'
    )
    expect(cli.execute { 'extract', path, '../evil', '--stdout' }).toEqual(
      failed 'archive entry not found: ../evil'
    )
    -- 1.1.0 looks for the entry before the target; the installed 0.1.0 looks the other way.
    expect(cli.execute { 'extract', path, 'textures', '-o', `{base}/in` }).toEqual(
      failed 'archive entry not found: textures'
    )
    expect((fs.exists(`{base}/out`))).toBe(false)

    -- The hex is read before the archive is opened.
    local odd = failed 'archive error: --entry-hex must contain an even number of hexadecimal digits'
    expect(cli.execute { 'extract', path, '--entry-hex', 'abc', '--stdout' }).toEqual(odd)
    expect(cli.execute { 'extract', path, '--entry-hex', 'abc', '-o', `{base}/out` }).toEqual(odd)
    expect(cli.execute { 'extract', missing, '--entry-hex', 'abc', '-o', `{base}/out` }).toEqual(odd)
    expect(cli.execute { 'extract', path, '--entry-hex', 'zz', '--stdout' }).toEqual(
      failed 'archive error: --entry-hex contains a non-hexadecimal digit'
    )

    expect(cli.execute { 'add', path, `{base}/in2`, '-o', path }).toEqual(
      failed 'archive error: explicit output archive path must differ from input archive path; omit output to replace the input archive'
    )
    expect(cli.execute { 'add', path, `{base}/in2`, `{base}/in2/added.txt` }).toEqual(
      failed(
        `archive error: duplicate archive path after normalization: added.txt (first source: {base}/in2/added.txt, duplicate source: {base}/in2/added.txt)`
      )
    )
    expect(cli.execute { 'add', path, `{base}/nowhere` }).toEqual(
      failed 'I/O error: No such file or directory (os error 2)'
    )
    expect(digest(path)).toBe '175 347e79bf51bcdbc2'
  end)

  suite.it('names the command line\'s options in what create refuses', function()
    local base = fixture()
    local input, output = `{base}/in`, `{base}/out.archive`
    -- 1.1.0's wording, which its own tests pin; the installed 0.1.0 words these two otherwise.
    expect(cli.execute { 'create', output, input, '--format', 'tes3', '--compress' }).toEqual(
      failed 'archive error: --compress is not valid with --format tes3'
    )
    expect(cli.execute { 'create', output, input, '--format', 'tes3', '--ba2-kind', 'dx10' }).toEqual(
      failed 'archive error: --ba2-kind is not valid with --format tes3'
    )
    expect(
      cli.execute { 'create', output, input, '--format', 'tes3', '--tes4-version', 'oblivion', '--dry-run' }
    ).toEqual(failed 'archive error: --tes4-version is not valid with --format tes3')
    expect(cli.execute { 'create', output, input, '--format', 'tes4', '--ba2-version', 'starfield' }).toEqual(
      failed 'archive error: --ba2-version is not valid with --format tes4'
    )
    expect(cli.execute { 'create', output, input, '--format', 'ba2', '--tes4-version', 'skyrim' }).toEqual(
      failed 'archive error: --tes4-version is not valid with --format ba2'
    )
    expect(cli.execute { 'create', output, input, '--format', 'ba2', '--ba2-kind', 'gnmf' }).toEqual(
      failed 'archive error: creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive'
    )
    expect(cli.execute { 'create', output, input, '--format', 'ba2', '--ba2-kind', 'dx10' }).toEqual(
      failed 'archive error: DX10 BA2 archives can only contain .dds files: meshes/example.nif'
    )
    expect(cli.execute { 'create', output, `{base}/nowhere`, '--format', 'tes3' }).toEqual(
      failed 'I/O error: No such file or directory (os error 2)'
    )
    expect((fs.exists(output))).toBe(false)

    assert(fs.symlink('readme.txt', `{input}/link.txt`))
    local linked =
      failed(`refusing to follow symlink input path: {input}/link.txt; pass --follow-symlinks to opt in`)
    expect(cli.execute { 'create', output, input, '--format', 'tes3' }).toEqual(linked)
    expect(cli.execute { 'create', output, input, '--format', 'tes3', '--dry-run' }).toEqual(linked)
    expect((fs.exists(output))).toBe(false)
    expect(cli.execute { 'create', output, input, '--format', 'tes3', '--follow-symlinks' }).toEqual(
      done 'files: 4\n'
    )
    expect(cli.execute { 'list', output }).toEqual(
      done 'textures/example.dds\nmeshes/example.nif\nreadme.txt\nlink.txt\n'
    )
  end)
end)

suite.describe('run', function()
  suite.it('returns 0 for what it did, 8 for a command line it can\'t use, 9 for a failed run', function()
    local base = fixture()
    expect(cli.run { '--version' }).toBe(0)
    expect(cli.run { 'verify', `{base}/t3.bsa` }).toBe(0)
    expect(cli.run { 'extract-all', `{base}/t3.bsa`, '-o', `{base}/out` }).toBe(0)
    expect(read(`{base}/out/readme.txt`)).toBe 'readme'
    expect(cli.run { 'extract-all', `{base}/t3.bsa`, '-o', `{base}/out` }).toBe(9)
    expect(cli.run { 'info', `{base}/missing.bsa` }).toBe(9)
    expect(cli.run { 'info' }).toBe(8)
    expect(cli.run { 'create', `{base}/f.bsa`, `{base}/in`, '--format', 'tes3', '--fsync' }).toBe(8)
    expect((fs.exists(`{base}/f.bsa`))).toBe(false)
  end)
end)

return suite
