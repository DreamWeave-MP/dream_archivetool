--!strict
--!native
-- SPDX-License-Identifier: MIT OR Apache-2.0
--[[
@dream-archivetool/commands: what each of dream_archivetool's subcommands does, as a function
from one request to one result, for whatever front end asks: a command line, or a window.

  info(request) -> InfoResult
  list(request) -> ListResult
  verify(request) -> VerifyResult
  diff(request) -> DiffResult
  extract(request) -> ExtractResult
  extractAll(request) -> ExtractAllResult
  create(request) -> CreateResult
  add(request) -> AddResult

A request is plain data: host paths, what to select, and how. A result is plain data too, tagged
with its command's name: `@dream-archivetool`'s reports and plans as that module gives them, with
its names and spellings, and for a command that writes, what it decided and what it wrote. No
function here reads an argument list, the environment, or standard input, prints, lays text out,
or chooses an exit status.

The four commands that write plan before they write, and take `dryRun` to stop there. Their
result carries the plan either way. An extraction whose target is already there while `overwrite`
is 'fail' writes nothing and says so in `conflicts`, where the original stops with `target
already exists`. Everything else that keeps a request from being carried out is raised, as
`@dream-archivetool` words it: an archive that can't be opened, an entry that isn't there, a path
extraction refuses, an input a format can't hold.

`@dream-archivetool` can't flush what it writes to disk, so no request has the original's
`fsync`.
]]

local tool = require '@dream-archivetool'

export type Overwrite = tool.Overwrite

--- The archive families `create` writes, as the original's `--format` spells them.
export type Format = 'tes3' | 'tes4' | 'ba2'

export type Tes4Version = 'oblivion' | 'fallout3' | 'skyrim' | 'skyrimSe'

export type Ba2Kind = 'gnrl' | 'dx10' | 'gnmf'

export type Ba2Version = 'fallout4' | 'starfield' | 'fallout4NextGen'

export type InfoRequest = { archive: string }

export type InfoResult = { command: 'info', info: tool.ArchiveInfo }

export type ListRequest = { archive: string }

--- The archive's named entries, in archive order.
export type ListResult = { command: 'list', archive: string, entries: { tool.ArchiveEntry } }

export type VerifyRequest = {
  archive: string,
  --- Read every named entry's payload, to prove it can be.
  readPayloads: boolean?,
}

export type VerifyResult = { command: 'verify', report: tool.VerifyReport }

export type DiffRequest = {
  old: string,
  new: string,
  --- Compare payloads by their FNV-1a fingerprints, not the entries' sizes alone.
  fingerprintPayloads: boolean?,
}

export type DiffResult = { command: 'diff', report: tool.DiffReport }

export type ExtractRequest = {
  archive: string,
  --- The entry, as its path in the archive: any spelling the archive's lookup normalizes.
  entry: string?,
  --- The entry instead, as a `pathBytesHex` from a listing, a report, or a plan.
  entryHex: string?,
  --- The folder to extract into; the current one without it.
  output: string?,
  --- Write the entry under its file name alone, without the archive's folders.
  flat: boolean?,
  --- What to do when the target is already there; 'fail' without it.
  overwrite: Overwrite?,
  --- Give the entry's bytes back as `data` and write nothing: `output`, `flat`, and `overwrite`
  --- then have nothing to say, and are refused.
  toMemory: boolean?,
  dryRun: boolean?,
}

export type ExtractResult = {
  command: 'extract',
  dryRun: boolean,
  --- What the extraction decided before writing; nil only for `toMemory`.
  plan: tool.ExtractPlan?,
  --- The plan's entries whose target is there while `overwrite` is 'fail'. A run with any wrote
  --- nothing.
  conflicts: { tool.ExtractPlanRow },
  --- What was extracted and skipped; nil when nothing ran.
  summary: tool.Summary?,
  --- The targets written, as the plan displays them.
  written: { string },
  --- The entry's bytes, for `toMemory`.
  data: string?,
}

export type ExtractAllRequest = {
  archive: string,
  output: string?,
  overwrite: Overwrite?,
  dryRun: boolean?,
}

export type ExtractAllResult = {
  command: 'extractAll',
  dryRun: boolean,
  plan: tool.ExtractPlan,
  conflicts: { tool.ExtractPlanRow },
  summary: tool.Summary?,
  written: { string },
}

export type CreateRequest = {
  --- The archive to write.
  archive: string,
  --- A file, or a folder whose files are stored by their paths below it.
  input: string,
  format: Format,
  --- 'oblivion' without it; for 'tes4' alone.
  tes4Version: Tes4Version?,
  --- 'gnrl' without it; for 'ba2' alone.
  ba2Kind: Ba2Kind?,
  --- 'fallout4' without it; for 'ba2' alone.
  ba2Version: Ba2Version?,
  --- For 'tes4' and 'ba2'.
  compress: boolean?,
  --- Take what a symbolic link among the inputs points at, where a link is otherwise refused.
  followSymlinks: boolean?,
  dryRun: boolean?,
}

export type CreateResult = {
  command: 'create',
  dryRun: boolean,
  plan: tool.CreatePlan,
  --- How many files the archive written holds; nil for a dry run.
  files: number?,
  --- The archive written, when one was.
  written: { string },
}

export type AddRequest = {
  --- The archive to add to.
  archive: string,
  --- Files, and folders whose files are stored by their paths below each.
  inputs: { string },
  --- Write the result here and leave `archive` as it is; without it `archive` is replaced.
  output: string?,
  followSymlinks: boolean?,
  dryRun: boolean?,
}

export type AddResult = {
  command: 'add',
  dryRun: boolean,
  plan: tool.AddPlan,
  files: number?,
  written: { string },
}

export type Result =
  InfoResult
  | ListResult
  | VerifyResult
  | DiffResult
  | ExtractResult
  | ExtractAllResult
  | CreateResult
  | AddResult

local function fail(message: string): never return error(message, 0) end

local function info(request: InfoRequest): InfoResult
  return { command = 'info', info = tool.info(request.archive) }
end

local function list(request: ListRequest): ListResult
  return { command = 'list', archive = request.archive, entries = tool.list(request.archive) }
end

local function verify(request: VerifyRequest): VerifyResult
  return {
    command = 'verify',
    report = tool.verify(request.archive, { readPayloads = request.readPayloads }),
  }
end

local function diff(request: DiffRequest): DiffResult
  return {
    command = 'diff',
    report = tool.diff(
      request.old,
      request.new,
      { fingerprintPayloads = request.fingerprintPayloads }
    ),
  }
end

local function conflictsIn(plan: tool.ExtractPlan): { tool.ExtractPlanRow }
  return [for row in plan.entries if row.action == 'conflict' => row :: tool.ExtractPlanRow]
end

--- The targets a plan writes: the ones it extracts, and the ones it replaces.
local function targetsOf(plan: tool.ExtractPlan): { string }
  return [for {action, target} in plan.entries if action == 'extract' or action == 'overwrite' =>
    target]
end

local function extractToMemory(request: ExtractRequest): ExtractResult
  if request.output or request.flat or request.overwrite then
    fail 'extract: toMemory writes nothing, so it takes no output, flat, or overwrite'
  end

  local {entry, entryHex} = request
  local data = if entryHex
    then tool.readEntryByPathHex(request.archive, entryHex)
    else tool.readEntry(request.archive, entry :: string)
  return {
    command = 'extract',
    dryRun = request.dryRun == true,
    conflicts = {},
    written = {},
    data = data,
  }
end

local function extract(request: ExtractRequest): ExtractResult
  local {entry, entryHex} = request
  if entry and entryHex then fail 'extract: the entry is named twice, as entry and as entryHex' end

  if not entry and not entryHex then fail 'extract: no entry is named, as entry or as entryHex' end

  if request.toMemory then return extractToMemory(request) end

  local options: tool.ExtractOptions = {
    output = request.output,
    overwrite = request.overwrite,
    preservePaths = not request.flat,
  }

  -- The plan refuses what the extraction would, for the same reason: a path that isn't safe, and
  -- then an entry that isn't there.
  local plan = if entryHex
    then tool.planExtractByPathHex(request.archive, { entryHex }, options)
    else tool.planExtract(request.archive, { entry :: string }, options)
  local conflicts = conflictsIn(plan)
  if request.dryRun or #conflicts > 0 then
    return {
      command = 'extract',
      dryRun = request.dryRun == true,
      plan = plan,
      conflicts = conflicts,
      written = {},
    }
  end

  local summary = if entryHex
    then tool.extractByPathHex(request.archive, entryHex, options)
    else tool.extract(request.archive, entry :: string, options)
  return {
    command = 'extract',
    dryRun = false,
    plan = plan,
    conflicts = conflicts,
    summary = summary,
    written = targetsOf(plan),
  }
end

local function extractAll(request: ExtractAllRequest): ExtractAllResult
  local options: tool.ExtractAllOptions = { output = request.output, overwrite = request.overwrite }
  local plan = tool.planExtractAll(request.archive, options)
  local conflicts = conflictsIn(plan)
  if request.dryRun or #conflicts > 0 then
    return {
      command = 'extractAll',
      dryRun = request.dryRun == true,
      plan = plan,
      conflicts = conflicts,
      written = {},
    }
  end

  return {
    command = 'extractAll',
    dryRun = false,
    plan = plan,
    conflicts = conflicts,
    summary = tool.extractAll(request.archive, options),
    written = targetsOf(plan),
  }
end

local function create(request: CreateRequest): CreateResult
  local options: tool.CreateOptions = {
    format = request.format,
    tes4Version = request.tes4Version,
    ba2Kind = request.ba2Kind,
    ba2Version = request.ba2Version,
    compress = request.compress,
    followSymlinks = request.followSymlinks,
  }

  local plan = tool.planCreate(request.archive, request.input, options)
  if request.dryRun then return { command = 'create', dryRun = true, plan = plan, written = {} } end

  local made = tool.create(request.archive, request.input, options)
  return {
    command = 'create',
    dryRun = false,
    plan = plan,
    files = made.files,
    written = { plan.output },
  }
end

local function add(request: AddRequest): AddResult
  local options: tool.AddOptions = {
    inputs = request.inputs,
    output = request.output,
    followSymlinks = request.followSymlinks,
  }

  local plan = tool.planAdd(request.archive, options)
  if request.dryRun then return { command = 'add', dryRun = true, plan = plan, written = {} } end

  local made = tool.add(request.archive, options)
  return {
    command = 'add',
    dryRun = false,
    plan = plan,
    files = made.files,
    written = { plan.output },
  }
end

return table.freeze {
  info = info,
  list = list,
  verify = verify,
  diff = diff,
  extract = extract,
  extractAll = extractAll,
  create = create,
  add = add,
}
