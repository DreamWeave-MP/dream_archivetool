--!strict
--!native
-- SPDX-License-Identifier: MIT OR Apache-2.0
--[[
@dream-archivetool/cli: dream_archivetool's command line, over @dream-archivetool/commands.

  parse(argv) -> (command, nil) | (nil, message)
  render(result, options) -> standard output's text
  status(result) -> exit status
  diagnostic(result) -> standard error's message, or nil
  errorMessage(problem) -> a raised problem in the command line's words
  execute(argv) -> { status, stdout, stderr }
  run(argv) -> exit status

  dream-archivetool [OPTIONS] [COMMAND]

The command is the package's name; the original's is dream_archivetool. Its subcommands, flags,
short forms, and output are dream_archivetool 1.1.0's, each flag set on the request field of
@dream-archivetool/commands it stands for: `parse` reads an argument list into the command it
names and that command's request without touching a file, or says what it couldn't read. A flag
is spelled `--name value`, `--name=value`, `-n value`, `-nvalue`, or `-n=value`; flags that take
no value can share a dash; `--` ends the flags. No flag can be given twice.

`render` is a result as the original prints it, and `status` and `diagnostic` what the original
exits with and says for it; none of the three reads or writes anything. `execute` is the whole
run with its output held as text, and `run` writes that out.

Three of the original's flags stand for things this port doesn't do and are refused by name:
--generate-completion and --generate-manpage, and --fsync, which @dream-archivetool refuses
because @dream/fs can't flush a file and its folder to disk. The original takes an entry path
that isn't UTF-8 as an argument; `parse` does too, but Jess hands a command target only UTF-8
arguments, so from a shell such an entry is named with --entry-hex.

Exit statuses: 0 when the command did what it was asked; 8 when the command line can't be used
(an unknown command or flag, a missing or extra argument, a value a flag doesn't take, flags that
can't be used together), said on standard error as `dream-archivetool: <message>`; 9 when the run
failed, said as `ERROR: <message>` in the original's words. The original exits 2 and 1 for those.
]]

local commands = require '@dream-archivetool/commands'
local process = require '@dream/process'
local tool = require '@dream-archivetool'

local rendering = require './render'
local usage = require './usage'

export type Options = rendering.Options

export type Output = { status: number, stdout: string, stderr: string }

--- An argument list as what it asks for: a subcommand with its request and how to print the
--- result, the help for a subcommand or for the command itself, or the version.
export type Command =
  { action: 'help', topic: string? }
  | { action: 'version' }
  | { action: 'info', request: commands.InfoRequest, options: Options }
  | { action: 'list', request: commands.ListRequest, options: Options }
  | { action: 'verify', request: commands.VerifyRequest, options: Options }
  | { action: 'diff', request: commands.DiffRequest, options: Options }
  | { action: 'extract', request: commands.ExtractRequest, options: Options }
  | { action: 'extract-all', request: commands.ExtractAllRequest, options: Options }
  | { action: 'create', request: commands.CreateRequest, options: Options }
  | { action: 'add', request: commands.AddRequest, options: Options }

--- A flag: its spellings, what it sets, the name of the value it takes when it takes one, the
--- values that can be, what it can't be given with, and why it is refused when it is.
type Flag = {
  long: string,
  short: string?,
  sets: string,
  value: string?,
  choices: { string }?,
  conflicts: { string }?,
  why: string?,
}

--- A subcommand's arguments, in order: what clap calls each, whether it has to be given, and
--- whether it takes every argument that is left.
type Positional = { name: string, required: boolean, rest: boolean? }

--- What a subcommand's argument list held: its arguments in order, each flag's value by what
--- the flag sets, and every flag given, by what it sets.
type Given = {
  positional: { string },
  values: { [string]: string },
  flags: { [string]: boolean },
}

type Spec = {
  positionals: { Positional },
  flags: { Flag },
  build: (given: Given) -> (Command?, string?),
}

local Exit = table.freeze { ok = 0, invalidInput = 8, runtime = 9 }

local NoSync =
  '@dream/fs can\'t flush a file and its folder to disk; without it a file is still written whole beside its target and renamed into place'

--- The original's flags this port has nothing behind, wherever they are given.
local Absent: { [string]: string } = {
  ['generate-completion'] = 'it has no shell completion scripts to generate',
  ['generate-manpage'] = 'it has no manpage to generate',
}

local Json: Flag = { long = 'json', sets = 'json' }
local Fsync: Flag = { long = 'fsync', sets = 'fsync', why = NoSync }
local DryRun: Flag = { long = 'dry-run', sets = 'dryRun' }
local FollowSymlinks: Flag = { long = 'follow-symlinks', sets = 'followSymlinks' }

--- What each of `create`'s enumerated values stands for in its request, by the flag's spelling
--- of it.
local Formats: { [string]: commands.Format } = { tes3 = 'tes3', tes4 = 'tes4', ba2 = 'ba2' }

local Tes4Versions: { [string]: commands.Tes4Version } = {
  oblivion = 'oblivion',
  fallout3 = 'fallout3',
  skyrim = 'skyrim',
  ['skyrim-se'] = 'skyrimSe',
}

local Ba2Kinds: { [string]: commands.Ba2Kind } = { gnrl = 'gnrl', dx10 = 'dx10', gnmf = 'gnmf' }

local Ba2Versions: { [string]: commands.Ba2Version } = {
  fallout4 = 'fallout4',
  starfield = 'starfield',
  ['fallout4-next-gen'] = 'fallout4NextGen',
}

--- The overwrite policy --overwrite and --skip-existing stand for, which can't both be given.
local function overwrite(given: Given): commands.Overwrite?
  if given.flags.overwrite then return 'overwrite' end

  return if given.flags.skipExisting then 'skip' else nil
end

local Specs: { [string]: Spec } = {
  info = {
    positionals = { { name = '<ARCHIVE>', required = true } },
    flags = { Json },
    build = function(given: Given): (Command?, string?)
      return {
        action = 'info' :: 'info',
        request = { archive = given.positional[1] },
        options = { json = given.flags.json },
      },
        nil
    end,
  },

  list = {
    positionals = { { name = '<ARCHIVE>', required = true } },
    flags = {
      { long = 'long', short = 'l', sets = 'long', conflicts = { 'json' } },
      Json,
    },

    build = function(given: Given): (Command?, string?)
      return {
        action = 'list' :: 'list',
        request = { archive = given.positional[1] },
        options = { json = given.flags.json, long = given.flags.long },
      },
        nil
    end,
  },

  verify = {
    positionals = { { name = '<ARCHIVE>', required = true } },
    flags = { { long = 'read-payloads', sets = 'readPayloads' }, Json },
    build = function(given: Given): (Command?, string?)
      return {
        action = 'verify' :: 'verify',
        request = { archive = given.positional[1], readPayloads = given.flags.readPayloads },
        options = { json = given.flags.json },
      },
        nil
    end,
  },

  diff = {
    positionals = {
      { name = '<OLD>', required = true },
      { name = '<NEW>', required = true },
    },

    flags = { { long = 'hash', sets = 'hash' }, Json },
    build = function(given: Given): (Command?, string?)
      return {
        action = 'diff' :: 'diff',
        request = {
          old = given.positional[1],
          new = given.positional[2],
          fingerprintPayloads = given.flags.hash,
        },

        options = { json = given.flags.json },
      },
        nil
    end,
  },

  extract = {
    positionals = {
      { name = '<ARCHIVE>', required = true },
      { name = '[ENTRY]', required = false },
    },

    flags = {
      { long = 'entry-hex', sets = 'entryHex', value = 'HEX', conflicts = { 'entry' } },
      { long = 'output', short = 'o', sets = 'output', value = 'OUTPUT', conflicts = { 'stdout' } },
      {
        long = 'stdout',
        sets = 'stdout',
        conflicts = { 'output', 'flat', 'overwrite', 'skipExisting' },
      },
      Fsync,
      { long = 'flat', sets = 'flat', conflicts = { 'stdout' } },
      { long = 'overwrite', sets = 'overwrite', conflicts = { 'skipExisting', 'stdout' } },
      { long = 'skip-existing', sets = 'skipExisting', conflicts = { 'overwrite', 'stdout' } },
      { long = 'json', sets = 'json', conflicts = { 'stdout' } },
    },

    build = function(given: Given): (Command?, string?)
      local entry, entryHex = given.positional[2], given.values.entryHex
      if not entry and not entryHex then
        return nil, 'extract requires [ENTRY] or --entry-hex <HEX>'
      end

      return {
        action = 'extract' :: 'extract',
        request = {
          archive = given.positional[1],
          entry = entry,
          entryHex = entryHex,
          output = given.values.output,
          flat = given.flags.flat,
          overwrite = overwrite(given),
          toMemory = given.flags.stdout,
        },

        options = { json = given.flags.json },
      },
        nil
    end,
  },

  ['extract-all'] = {
    positionals = { { name = '<ARCHIVE>', required = true } },
    flags = {
      { long = 'output', short = 'o', sets = 'output', value = 'OUTPUT' },
      { long = 'overwrite', sets = 'overwrite', conflicts = { 'skipExisting' } },
      { long = 'skip-existing', sets = 'skipExisting', conflicts = { 'overwrite' } },
      Json,
      DryRun,
      Fsync,
    },

    build = function(given: Given): (Command?, string?)
      return {
        action = 'extract-all' :: 'extract-all',
        request = {
          archive = given.positional[1],
          output = given.values.output,
          overwrite = overwrite(given),
          dryRun = given.flags.dryRun,
        },

        options = { json = given.flags.json },
      },
        nil
    end,
  },

  create = {
    positionals = {
      { name = '<ARCHIVE>', required = true },
      { name = '<INPUT>', required = true },
    },

    flags = {
      { long = 'format', sets = 'format', value = 'FORMAT', choices = { 'tes3', 'tes4', 'ba2' } },
      {
        long = 'tes4-version',
        sets = 'tes4Version',
        value = 'TES4_VERSION',
        choices = { 'oblivion', 'fallout3', 'skyrim', 'skyrim-se' },
      },
      {
        long = 'ba2-kind',
        sets = 'ba2Kind',
        value = 'BA2_KIND',
        choices = { 'gnrl', 'dx10', 'gnmf' },
      },
      {
        long = 'ba2-version',
        sets = 'ba2Version',
        value = 'BA2_VERSION',
        choices = { 'fallout4', 'starfield', 'fallout4-next-gen' },
      },
      { long = 'compress', sets = 'compress' },
      Json,
      DryRun,
      Fsync,
      FollowSymlinks,
    },

    build = function(given: Given): (Command?, string?)
      local format: commands.Format? = Formats[given.values.format]
      if not format then return nil, 'create requires --format <FORMAT>' end

      return {
        action = 'create' :: 'create',
        request = {
          archive = given.positional[1],
          input = given.positional[2],
          format = format,
          tes4Version = Tes4Versions[given.values.tes4Version],
          ba2Kind = Ba2Kinds[given.values.ba2Kind],
          ba2Version = Ba2Versions[given.values.ba2Version],
          compress = given.flags.compress,
          followSymlinks = given.flags.followSymlinks,
          dryRun = given.flags.dryRun,
        },

        options = { json = given.flags.json },
      },
        nil
    end,
  },

  add = {
    positionals = {
      { name = '<ARCHIVE>', required = true },
      { name = '<INPUTS>...', required = true, rest = true },
    },

    flags = {
      { long = 'output', short = 'o', sets = 'output', value = 'OUTPUT' },
      Json,
      DryRun,
      Fsync,
      FollowSymlinks,
    },

    build = function(given: Given): (Command?, string?)
      return {
        action = 'add' :: 'add',
        request = {
          archive = given.positional[1],
          inputs = table.move(given.positional, 2, #given.positional, 1, {}),
          output = given.values.output,
          followSymlinks = given.flags.followSymlinks,
          dryRun = given.flags.dryRun,
        },

        options = { json = given.flags.json },
      },
        nil
    end,
  },
}

--- `text` before and after its first `=`, or nil when it has none.
local function pair(text: string): (string?, string?)
  local found = string.find(text, '=', 1, true)
  if not found then return nil, nil end

  return string.sub(text, 1, found - 1), string.sub(text, found + 1)
end

--- Whether an argument is spelled as a flag: a dash and something after it.
local function flagLike(argument: string): boolean
  return #argument > 1 and string.byte(argument, 1) == 45
end

local function refusal(name: string, reason: string): string
  return `--{name} is not part of this port: {reason}`
end

--- How a conflict names what a flag sets: the flag, or `extract`'s entry argument.
local function label(spec: Spec, sets: string): string
  for _, flag in spec.flags do
    if flag.sets == sets then return `--{flag.long}` end
  end

  return '[ENTRY]'
end

--- The first two flags given that can't be given together, in the order the subcommand lists
--- its flags, as a message; nil when there are none.
local function conflict(spec: Spec, given: Given): string?
  for _, flag in spec.flags do
    local others = flag.conflicts
    if not others or not given.flags[flag.sets] then continue end

    for _, other in others do
      if given.flags[other] then
        return `--{flag.long} can't be used with {label(spec, other)}`
      end
    end
  end

  return nil
end

--- The required arguments an argument list left out, as a message; nil when it has them all.
local function missing(name: string, spec: Spec, given: Given): string?
  local absent: { string } = {}
  for index, positional in spec.positionals do
    if positional.required and not given.positional[index] then
      table.insert(absent, positional.name)
    end
  end

  if #absent == 0 then return nil end

  return `{name} requires {table.concat(absent, ' and ')}`
end

--- A subcommand's arguments, `argv` from `start` on, as its command.
local function parseCommand(name: string, argv: { string }, start: number): (Command?, string?)
  local spec = Specs[name]
  local long: { [string]: Flag }, short: { [string]: Flag } = {}, {}
  for _, flag in spec.flags do
    long[flag.long] = flag
    if flag.short then short[flag.short] = flag end
  end

  local last = spec.positionals[#spec.positionals]
  local limit = if last.rest then math.huge else #spec.positionals
  local given: Given = { positional = {}, values = {}, flags = {} }
  local index = start
  local literal = false

  --- Takes one flag as `spelling` gave it, with the value attached to it if one was, reading
  --- the next argument when it needs one. What it can't take comes back as the message.
  local function take(flag: Flag, spelling: string, attached: string?): string?
    if flag.why then return refusal(flag.long, flag.why) end

    if given.flags[flag.sets] then return `{spelling} can be given only once` end

    given.flags[flag.sets] = true
    if not flag.value then
      return if attached then `{spelling} takes no value` else nil
    end

    local value = attached or argv[index + 1]
    if not attached then
      if not value or flagLike(value) then return `{spelling} requires a value` end

      index += 1
    end

    if flag.choices and not table.find(flag.choices, value) then
      return `{spelling} takes {table.concat(flag.choices, ', ')}, not '{value}'`
    end

    given.values[flag.sets] = value
    return nil
  end

  while index <= #argv do
    local argument = argv[index]

    if literal or not flagLike(argument) then
      if #given.positional >= limit then return nil, `unexpected argument '{argument}'` end

      table.insert(given.positional, argument)
    elseif argument == '--' then
      literal = true
    elseif string.sub(argument, 1, 2) == '--' then
      local body = string.sub(argument, 3)
      local before, attached = pair(body)
      local spelled = before or body
      if spelled == 'help' then return { action = 'help' :: 'help', topic = name }, nil end

      local flag = long[spelled]
      if not flag and Absent[spelled] then return nil, refusal(spelled, Absent[spelled]) end

      if not flag then return nil, `unknown option '--{spelled}'` end

      local refused = take(flag, `--{flag.long}`, attached)
      if refused then return nil, refused end
    else
      -- Flags that share a dash, the last of which may take what follows it as its value.
      local position = 2

      while position <= #argument do
        local letter = string.sub(argument, position, position)
        if letter == 'h' then return { action = 'help' :: 'help', topic = name }, nil end

        local flag = short[letter]
        if not flag then return nil, `unknown option '-{letter}'` end

        local attached: string? = nil
        if flag.value and position < #argument then
          local rest = string.sub(argument, position + 1)
          attached = if string.sub(rest, 1, 1) == '=' then string.sub(rest, 2) else rest
        end

        if flag.value then position = #argument end

        local refused = take(flag, `-{letter}`, attached)
        if refused then return nil, refused end

        position += 1
      end
    end

    index += 1
  end

  local absent = missing(name, spec, given)
  if absent then return nil, absent end

  -- `extract` names its entry by an argument or by --entry-hex, and not by both.
  if name == 'extract' and given.positional[2] then given.flags.entry = true end

  local conflicting = conflict(spec, given)
  if conflicting then return nil, conflicting end

  return spec.build(given)
end

--- `help`'s arguments: nothing, or the one subcommand to describe.
local function parseHelp(argv: { string }, start: number): (Command?, string?)
  local topic = argv[start]
  if argv[start + 1] then
    return nil, `help takes one command, not '{argv[start + 1]}' as well`
  end

  if not topic or topic == 'help' or topic == '--help' or topic == '-h' then
    return { action = 'help' :: 'help' }, nil
  end

  if flagLike(topic) then return nil, `unknown option '{topic}'` end

  if not Specs[topic] then return nil, `unknown command '{topic}'` end

  return { action = 'help' :: 'help', topic = topic }, nil
end

local function parse(argv: { string }): (Command?, string?)
  local argument = argv[1]
  -- Without a command the original prints its help and exits 0.
  if not argument then return { action = 'help' :: 'help' }, nil end

  if string.sub(argument, 1, 2) == '--' then
    local body = string.sub(argument, 3)
    local spelled = pair(body) or body
    if spelled == 'help' then return { action = 'help' :: 'help' }, nil end

    if spelled == 'version' then return { action = 'version' :: 'version' }, nil end

    if Absent[spelled] then return nil, refusal(spelled, Absent[spelled]) end

    return nil, `unknown option '--{spelled}'`
  end

  if flagLike(argument) then
    local letter = string.sub(argument, 2, 2)
    if letter == 'h' then return { action = 'help' :: 'help' }, nil end

    if letter == 'V' then return { action = 'version' :: 'version' }, nil end

    return nil, `unknown option '-{letter}'`
  end

  if argument == 'help' then return parseHelp(argv, 2) end

  if not Specs[argument] then return nil, `unknown command '{argument}'` end

  return parseCommand(argument, argv, 2)
end

--- The first target that was already there, for an extraction that wrote nothing because of it:
--- where the original stops. Nil for a dry run, which only reports it, and for any other result.
local function stoppedAt(result: commands.Result): tool.ExtractPlanRow?
  if result.command == 'extract' and not result.dryRun then return result.conflicts[1] end

  if result.command == 'extractAll' and not result.dryRun then return result.conflicts[1] end

  return nil
end

--- The exit status the original ends with for `result`, as this command numbers it: 9 for an
--- extraction that stopped at a target already there, else 0.
local function status(result: commands.Result): number
  return if stoppedAt(result) then Exit.runtime else Exit.ok
end

--- What the original says on standard error for `result`, after `ERROR: `: the target an
--- extraction stopped at. Nil for a result with nothing to say there.
local function diagnostic(result: commands.Result): string?
  local row = stoppedAt(result)
  if not row then return nil end

  return `target already exists: {row.target}`
end

--- A problem `@dream-archivetool` raised, in the command line's words where the library names
--- its own options: the flags stand for them here, as they do in the original's messages.
local function errorMessage(problem: string): string
  local option, format = string.match(problem, '^(%a[%w]*) is not valid with format (%w+)$')
  if option and format and rendering.Formats[format] then
    return `archive error: --{rendering.kebab(option)} is not valid with --format {rendering.Formats[format]}`
  end

  local link =
    string.match(problem, '^refusing to follow symlink input path: (.*); set followSymlinks to opt in$')
  if link then
    return `refusing to follow symlink input path: {link}; pass --follow-symlinks to opt in`
  end

  local digits =
    string.match(problem, 'invalid pathBytesHex: archive error: archive path hex (.*)$')
  if digits then return `archive error: --entry-hex {digits}` end

  return problem
end

--- Runs the subcommand `command` names on its request.
local function perform(command: Command): commands.Result
  if command.action == 'info' then return commands.info(command.request) end

  if command.action == 'list' then return commands.list(command.request) end

  if command.action == 'verify' then return commands.verify(command.request) end

  if command.action == 'diff' then return commands.diff(command.request) end

  if command.action == 'extract' then return commands.extract(command.request) end

  if command.action == 'extract-all' then return commands.extractAll(command.request) end

  if command.action == 'create' then return commands.create(command.request) end

  if command.action == 'add' then return commands.add(command.request) end

  return error(`'{command.action}' names no subcommand to run`, 0)
end

--- Everything `run` does short of writing: the exit status, and what goes to each stream.
local function execute(argv: { string }): Output
  local command, message = parse(argv)
  if not command then
    return { status = Exit.invalidInput, stdout = '', stderr = `dream-archivetool: {message}\n` }
  end

  if command.action == 'help' then
    return { status = Exit.ok, stdout = `{usage.of(command.topic)}\n`, stderr = '' }
  end

  if command.action == 'version' then
    return { status = Exit.ok, stdout = `dream-archivetool {tool.version}\n`, stderr = '' }
  end

  -- A command raises when its request can't be carried out, where the original exits 1.
  local ran, result = pcall(perform, command)
  if not ran then
    return {
      status = Exit.runtime,
      stdout = '',
      stderr = `ERROR: {errorMessage(tostring(result))}\n`,
    }
  end

  local problem = diagnostic(result)
  return {
    status = status(result),
    stdout = rendering.render(result, command.options),
    stderr = if problem then `ERROR: {problem}\n` else '',
  }
end

local function run(argv: { string }): number
  local output = execute(argv)
  if output.stdout ~= '' then process.write('stdout', output.stdout) end

  if output.stderr ~= '' then process.write('stderr', output.stderr) end

  return output.status
end

return table.freeze {
  Exit = Exit,
  parse = parse,
  render = rendering.render,
  status = status,
  diagnostic = diagnostic,
  errorMessage = errorMessage,
  execute = execute,
  run = run,
}
