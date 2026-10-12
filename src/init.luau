--!strict
--!native
-- SPDX-License-Identifier: MIT OR Apache-2.0
-- @dream-archivetool: archive policy over `@dream-archive`, a port of the dream_archivetool
-- crate's `@dream/archivetool` module: info, verify, diff, extraction that refuses unsafe paths
-- and plans it, and creating and adding to archives with plans. Every function takes an archive
-- as a host path or as one `@dream-archive` opened, and returns plain tables; errors read as
-- the crate's do. `list`, `readEntry`, and `readEntryByPathHex` are the crate's
-- `ArchiveTool::list` and `read_entry_by_path_bytes`, which its Luau module left to
-- `@dream/archive` and its command line calls.
local archives = require '@dream-archive/backend'
local ba2 = require '@dream-archive/ba2'
local bytes = require '@dream/bytes'
local open = require '@dream-archive'
local tes3 = require '@dream-archive/bsa/tes3'
local tes4 = require '@dream-archive/bsa/tes4'
local types = require '@dream-archive/types'
local util = require '@dream-archive/util'
local fs = require '@dream/fs'

type Archive = types.Archive
type Backend = types.Backend

export type Overwrite = 'fail' | 'overwrite' | 'skip'

export type ArchiveInfo = {
  path: string,
  format: types.Format,
  fileCount: number,
  namedEntryCount: number,
  hasUnnameableEntries: boolean,
  rewritable: boolean,
  rewriteBlocker: string?,
  tes4: {
    version: string,
    archiveTypes: string,
    archiveTypesBits: number,
    archiveFlags: { string },
    archiveFlagsBits: number,
    unsupportedArchiveFlagsBits: number,
    nameMode: string,
  }?,
  ba2: { version: string, payloadFormat: string, compressionFormat: string, strings: boolean }?,
}

--- One named entry of an archive, as `list` gives it: its normalized path for display, that
--- path's bytes as the lookup key every `pathBytesHex` is, and the sizes the format records.
export type ArchiveEntry = {
  path: string,
  pathBytesHex: string,
  size: number?,
  compressedSize: number?,
}

export type PathIssue = {
  path: string,
  pathBytesHex: string,
  rawPathBytesHex: string?,
  collidingRawPathBytesHex: string?,
}

export type VerifyReport = {
  path: string,
  format: types.Format,
  fileCount: number,
  namedEntryCount: number,
  unnameableEntries: number,
  rewritable: boolean,
  rewriteBlocker: string?,
  duplicateNormalizedPaths: { PathIssue },
  unsafePaths: { PathIssue },
  payloadsRead: number?,
  warnings: { string },
}

export type DiffState = { size: number?, compressedSize: number?, payloadFingerprint: integer? }

export type DiffEntry = {
  path: string,
  pathBytesHex: string,
  size: number?,
  compressedSize: number?,
  payloadFingerprint: integer?,
}

export type DiffChange = { path: string, pathBytesHex: string, old: DiffState, new: DiffState }

export type DiffReport = {
  old: string,
  new: string,
  comparison: 'metadataOnly' | 'payloadFingerprint',
  fingerprintPayloads: boolean,
  added: { DiffEntry },
  removed: { DiffEntry },
  changed: { DiffChange },
  unchanged: number,
}

export type ExtractPlanRow = {
  action: 'extract' | 'overwrite' | 'skip' | 'conflict',
  path: string,
  pathBytesHex: string,
  target: string,
}

export type ExtractPlan = {
  operation: 'extract' | 'extractAll',
  archive: string,
  output: string,
  entries: { ExtractPlanRow },
}

export type ArchivePlanRow = {
  action: 'add' | 'replace' | 'preserve',
  source: string?,
  path: string,
  pathBytesHex: string,
  size: number?,
}

export type CreatePlan = {
  operation: 'create',
  format: types.Format,
  output: string,
  files: number,
  entries: { ArchivePlanRow },
}

export type AddPlan = {
  operation: 'add',
  archive: string,
  output: string,
  format: types.Format,
  files: number,
  added: number,
  replaced: number,
  preserved: number,
  entries: { ArchivePlanRow },
}

export type Summary = { extracted: number, skipped: number }

export type ExtractOptions = {
  output: string?,
  overwrite: Overwrite?,
  preservePaths: boolean?,
  fsync: boolean?,
}

export type ExtractAllOptions = { output: string?, overwrite: Overwrite?, fsync: boolean? }

export type CreateOptions = {
  format: string?,
  tes4Version: string?,
  ba2Kind: string?,
  ba2Version: string?,
  compress: boolean?,
  fsync: boolean?,
  followSymlinks: boolean?,
}

export type AddOptions = {
  inputs: { string },
  output: string?,
  fsync: boolean?,
  followSymlinks: boolean?,
}

--- An archive argument: a host path, or an archive `@dream-archive` opened.
export type Source = string | Archive

--- An entry argument: an archive path, or an entry of an opened archive.
export type EntryArgument = string | types.Entry

--- An entries argument: archive paths, entries such as `archive:entries()`, or both.
export type EntryList = { string } | types.Entries | { EntryArgument }

local fail = util.fail

--- The dream_archivetool release this port follows.
local Version = '1.1.0'

local UnnameableBlocker =
  'archive contains entries without recoverable paths; refusing to rewrite it lossy'
local HashOnlyBlocker =
  'TES4 hash-only archives do not have recoverable path names; refusing to rewrite them lossy'
local UnsupportedFlagsBlocker =
  'TES4 archive uses header flag bits this tool cannot preserve; refusing to rewrite it lossy'
local GnmfBlocker =
  'creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive'

local Tes4Flags = table.freeze {
  directoryStrings = 0x1,
  fileStrings = 0x2,
  compressed = 0x4,
  embeddedFileNames = 0x100,
}

--- The flags `info` names, in the order it names them.
local NamedTes4Flags = {
  { name = 'directoryStrings', bit = Tes4Flags.directoryStrings },
  { name = 'fileStrings', bit = Tes4Flags.fileStrings },
  { name = 'compressed', bit = Tes4Flags.compressed },
  { name = 'embeddedFileNames', bit = Tes4Flags.embeddedFileNames },
}

local RewritableTes4Flags = 0x107

local Tes4Versions: { [string]: number } =
  { oblivion = 103, fallout3 = 104, skyrim = 104, skyrimSe = 105, sse = 105 }

local Ba2Versions: { [string]: number } = { fallout4 = 1, starfield = 2, fallout4NextGen = 8 }

local Ba2Kinds: { [string]: boolean } = { gnrl = true, dx10 = true, gnmf = true }

local Formats: { [string]: types.Format } =
  { tes3 = 'bsaTes3', bsaTes3 = 'bsaTes3', tes4 = 'bsaTes4', bsaTes4 = 'bsaTes4', ba2 = 'ba2' }

local function archiveError(message: string): never return fail(`archive error: {message}`) end

local function ioError(message: unknown): never
  return fail(`I/O error: {util.hostMessage(message)}`)
end

local function unsafePath(path: string): never
  return fail(`unsafe archive path: {util.lossy(path)}`)
end

local function entryNotFound(path: string): never
  return fail(`archive entry not found: {util.lossy(path)}`)
end

local function targetExists(path: string): never
  return fail(`target already exists: {util.lossy(path)}`)
end

--- A failed read of a member as the crate sorts it: the archive's I/O failures are I/O errors,
--- a member it doesn't have is not found, and everything else is the archive's.
local function readError(key: string, message: unknown): never
  local text = tostring(message)
  if string.sub(text, 1, 26) == 'archive member not found: ' then return entryNotFound(key) end

  if
    text == util.Messages.endOfFile
    or text == util.Messages.shortRead
    or string.find(text, '%(os error %d+%)$')
  then
    return ioError(text)
  end

  return archiveError(text)
end

local function argumentCount(what: string, minimum: number, count: number)
  if count >= minimum then return end

  fail(`dream.archivetool.{what}: bad argument count (expected at least {minimum}, got {count})`)
end

local function hostPath(what: string, value: unknown, position: number): string
  return util.checkHostPath(`dream.archivetool.{what}`, value, position)
end

--- An archive argument as the binding converts it, before anything else is read: an archive
--- opened already, or a host path.
local function checkSource(what: string, value: unknown, position: number): Source
  if archives.labelOf(value) then return value :: Archive end

  return hostPath(what, value, position)
end

--- The archive a checked argument names, opened for this call when it is a path, and its label.
local function openSource(source: Source): (Archive, string)
  if type(source) ~= 'string' then
    local opened = source :: Archive
    return opened, archives.labelOf(opened) :: string
  end

  local path = source :: string
  -- openPath throws the crate's reason an archive can't be opened.
  local ok, result = pcall(open.openPath, path)
  if not ok then archiveError(`failed to open archive '{path}': {util.hostMessage(result)}`) end

  return result :: Archive, path
end

local function backendOf(archive: Archive): Backend return archives.backendOf(archive) :: Backend end

type Reader = { context: string, values: { [string]: unknown }, seen: { [string]: boolean } }

--- An option table read the way l3i's `Options` reads one: every key a string, each field
--- converted and named when it fails, and the keys nothing asked for refused after the rest.
local function readOptions<T>(context: string, value: unknown, body: (Reader) -> T): T
  if value == nil then return body { context = context, values = {}, seen = {} } end

  if type(value) ~= 'table' then fail(`{context}: options must be a table, got {type(value)}`) end

  local values: { [string]: unknown } = {}
  local keys: { string } = {}
  for key, field in value :: { [unknown]: unknown } do
    if type(key) ~= 'string' then
      fail(`{context}: option keys must be strings, got {type(key)}`)
    end

    local name = key :: string
    values[name] = field
    table.insert(keys, name)
  end

  local reader: Reader = { context = context, values = values, seen = {} }
  local result = body(reader)
  local unknown = [for key in keys if not reader.seen[key] => `'{key}'`]
  if #unknown == 0 then return result end

  local known = {}
  for key in reader.seen do
    table.insert(known, key)
  end

  table.sort(unknown)
  table.sort(known)
  local plural = if #unknown == 1 then '' else 's'
  local listed = if #known == 0 then 'none' else table.concat(known, ', ')
  return fail(
    `{context}: unknown option{plural} {table.concat(unknown, ', ')}; known options are {listed}`
  )
end

--- A field's raw value: nil when absent or nil.
local function field(reader: Reader, key: string): unknown
  reader.seen[key] = true
  return reader.values[key]
end

local function stringField(reader: Reader, key: string): string?
  local value = field(reader, key)
  if value == nil then return nil end

  if type(value) ~= 'string' then
    fail(`{reader.context}.{key}: expected string, got {type(value)}`)
  end

  return value :: string
end

local function booleanField(reader: Reader, key: string, default: boolean): boolean
  local value = field(reader, key)
  if value == nil then return default end

  if type(value) ~= 'boolean' then
    fail(`{reader.context}.{key}: expected boolean, got {type(value)}`)
  end

  return value :: boolean
end

--- The `fsync` option. `@dream/fs` doesn't expose directory syncing, so asking for it is
--- refused rather than ignored.
local function fsyncField(reader: Reader)
  if booleanField(reader, 'fsync', false) then
    fail(`{reader.context}.fsync: flushing to disk is not available in @dream-archivetool`)
  end
end

local function overwriteField(reader: Reader): Overwrite
  local value = stringField(reader, 'overwrite')
  if value == nil then return 'fail' end

  if value == 'fail' or value == 'overwrite' or value == 'skip' then return value :: Overwrite end

  return fail(`{reader.context}.overwrite: unknown overwrite mode: {value}`)
end

type Extraction = { output: string, overwrite: Overwrite, preservePaths: boolean }

local function extractOptions(context: string, value: unknown): Extraction
  return readOptions(context, value, function(reader: Reader): Extraction
    local output = stringField(reader, 'output') or '.'
    local overwrite = overwriteField(reader)
    local preservePaths = booleanField(reader, 'preservePaths', true)
    fsyncField(reader)
    return { output = output, overwrite = overwrite, preservePaths = preservePaths }
  end)
end

local function extractAllOptions(context: string, value: unknown): Extraction
  return readOptions(context, value, function(reader: Reader): Extraction
    local output = stringField(reader, 'output') or '.'
    local overwrite = overwriteField(reader)
    fsyncField(reader)
    return { output = output, overwrite = overwrite, preservePaths = true }
  end)
end

local function flagOption(context: string, value: unknown, key: string): boolean
  return readOptions(
    context,
    value,
    function(reader: Reader): boolean return booleanField(reader, key, false) end
  )
end

type Creation = {
  format: types.Format,
  tes4Version: number,
  ba2Kind: string,
  ba2Version: number,
  compress: boolean,
  followSymlinks: boolean,
}

local function createOptions(context: string, value: unknown): Creation
  return readOptions(context, value, function(reader: Reader): Creation
    local formatName = stringField(reader, 'format')
    local format: types.Format = 'bsaTes3'
    if formatName then
      local chosen = Formats[formatName]
      if not chosen then fail(`{context}.format: unknown archive format: {formatName}`) end

      format = chosen
    end

    local tes4Version = stringField(reader, 'tes4Version')
    local ba2Kind = stringField(reader, 'ba2Kind')
    local ba2Version = stringField(reader, 'ba2Version')
    local compress = booleanField(reader, 'compress', false)
    local function irrelevant(option: string, supplied: boolean)
      if supplied then fail(`{option} is not valid with format {format}`) end
    end

    if format == 'bsaTes3' then
      irrelevant('compress', compress)
      irrelevant('tes4Version', tes4Version ~= nil)
      irrelevant('ba2Kind', ba2Kind ~= nil)
      irrelevant('ba2Version', ba2Version ~= nil)
    elseif format == 'bsaTes4' then
      irrelevant('ba2Kind', ba2Kind ~= nil)
      irrelevant('ba2Version', ba2Version ~= nil)
    else
      irrelevant('tes4Version', tes4Version ~= nil)
    end

    local version = Tes4Versions[tes4Version or 'oblivion']
    if not version then fail(`unknown TES4 version: {tes4Version}`) end

    if not Ba2Kinds[ba2Kind or 'gnrl'] then fail(`unknown BA2 kind: {ba2Kind}`) end

    local ba2Number = Ba2Versions[ba2Version or 'fallout4']
    if not ba2Number then fail(`unknown BA2 version: {ba2Version}`) end

    fsyncField(reader)
    return {
      format = format,
      tes4Version = version,
      ba2Kind = ba2Kind or 'gnrl',
      ba2Version = ba2Number,
      compress = compress,
      followSymlinks = booleanField(reader, 'followSymlinks', false),
    }
  end)
end

--- `count` when `list` is a dense 1-based array, else the crate's error naming `path`.
local function denseLength(list: { [unknown]: unknown }, path: string): number
  local length = #list
  local seen = 0
  for key in list do
    local index = key :: number
    if type(key) ~= 'number' or index ~= math.floor(index) or index < 1 or index > length then
      fail(`{path} must be a dense 1-based array`)
    end

    seen += 1
  end

  if seen ~= length then fail(`{path} must not contain holes`) end

  return length
end

type Addition = { inputs: { string }, output: string?, followSymlinks: boolean }

local function addOptions(context: string, value: unknown): Addition
  if value == nil then fail(`{context}: missing options table`) end

  return readOptions(context, value, function(reader: Reader): Addition
    local listed = field(reader, 'inputs')
    if listed == nil then fail(`{context}: missing required option 'inputs'`) end

    if type(listed) ~= 'table' then fail(`{context}.inputs: expected table, got {type(listed)}`) end

    denseLength(listed :: { [unknown]: unknown }, `{context}.inputs: inputs`)
    local inputs: { string } = {}
    for index, input in listed :: { unknown } do
      if type(input) ~= 'string' or not utf8.len(input :: string) then
        fail(
          `{context}.inputs: inputs[{index}]: expected a UTF-8 host path string, got {type(input)}`
        )
      end

      table.insert(inputs, input :: string)
    end

    if #inputs == 0 then fail(`{context} requires at least one input path`) end

    local output = stringField(reader, 'output')
    fsyncField(reader)
    return {
      inputs = inputs,
      output = output,
      followSymlinks = booleanField(reader, 'followSymlinks', false),
    }
  end)
end

local function decodeHex(text: string, context: string): string
  if #text % 2 ~= 0 then
    fail(`{context}: invalid pathBytesHex: archive error: archive path hex must contain an even number of hexadecimal digits`)
  end

  if string.find(text, '[^%x]') then
    fail(`{context}: invalid pathBytesHex: archive error: archive path hex contains a non-hexadecimal digit`)
  end

  return buffer.tostring(bytes.fromHex(text))
end

--- Whether extraction may write `path`: not empty or absolute, no NUL, a file name somewhere,
--- and no `..` or `:` in any component, `/` and `\` both separating.
local function extractable(path: string): boolean
  if path == '' or string.find(path, '\0', 1, true) then return false end

  local first = string.byte(path, 1)
  if first == 47 or first == 92 or string.find(path, ':', 1, true) then return false end

  -- Without two dots together no component is `..`, and any other byte is in a file's name.
  if not string.find(path, '..', 1, true) and string.find(path, '[^/\\.]') then return true end

  local named = false
  for component in string.gmatch(path, '[^/\\]+') do
    if component == '..' then return false end

    if component ~= '.' then named = true end
  end

  return named
end

local function checkExtractable(path: string)
  if not extractable(path) then unsafePath(path) end
end

--- `path` normalized for lookup, refused when nothing is left of it or it holds a NUL.
local function lookupKey(path: string): string
  local key = util.lookupKey(path)
  if key == '' or string.find(key, '\0', 1, true) then unsafePath(key) end

  return key
end

local function safeKey(path: string): string
  checkExtractable(path)
  local key = lookupKey(path)
  checkExtractable(key)
  return key
end

local function join(root: string, component: string): string
  if root == '' then return component end

  if string.byte(root, -1) == 47 then return root .. component end

  return `{root}/{component}`
end

--- The folder `path` is in; `.` for a bare name, as the crate writes beside one.
local function parentDirectory(path: string): string
  -- A path that ends in a name, below a folder that ends in one, is cut at its last separator.
  if string.byte(path, -1) ~= 47 then
    local separator = bytes.rfind(path, '/')
    if not separator then return '.' end

    if separator == 0 then return '/' end

    if string.byte(path, separator) ~= 47 then return string.sub(path, 1, separator) end
  end

  local trimmed = string.match(path, '^(.-)/*$') :: string
  local at = string.find(trimmed, '/[^/]*$')
  if not at then return '.' end

  if at == 1 then return '/' end

  local folder = string.gsub(string.sub(trimmed, 1, at - 1), '/+$', '')
  return if folder == '' then '.' else folder
end

--- The last component of `path`, or nil for one ending in `..` or naming nothing.
local function fileName(path: string): string?
  -- What follows the last separator of a path that doesn't end in one is its last component.
  if string.byte(path, -1) ~= 47 then
    local separator = bytes.rfind(path, '/')
    local name = if separator then string.sub(path, separator + 2) else path
    if name == '..' then return nil end

    if name ~= '.' and name ~= '' then return name end
  end

  local last: string? = nil
  for component in string.gmatch(path, '[^/]+') do
    if component ~= '.' then last = component end
  end

  return if last == '..' then nil else last
end

--- The components `Path` compares by: empty ones and inner `.` dropped.
local function components(path: string): { string }
  local list = {}
  if string.byte(path, 1) == 47 then table.insert(list, '/') end

  for component in string.gmatch(path, '[^/]+') do
    if component == '.' and #list > 0 then continue end

    table.insert(list, component)
  end

  return list
end

--- Whether one path's components come before another's, as `Path` orders two paths.
local function componentsLess(leftParts: { string }, rightParts: { string }): boolean
  for index = 1, math.min(#leftParts, #rightParts) do
    if leftParts[index] ~= rightParts[index] then return leftParts[index] < rightParts[index] end
  end

  return #leftParts < #rightParts
end

local function pathLess(left: string, right: string): boolean
  return componentsLess(components(left), components(right))
end

local function samePath(left: string, right: string): boolean
  return not pathLess(left, right) and not pathLess(right, left)
end

--- `path` as its components joined, which two paths share exactly when `Path` compares them
--- equal. A path with nothing for `components` to drop is its own, behind the root's `/`.
local function pathIdentity(path: string): string
  if
    string.byte(path, -1) ~= 47
    and not string.find(path, '//', 1, true)
    and not string.find(path, '/.', 1, true)
  then
    return if string.byte(path, 1) == 47 then '/' .. path else path
  end

  return table.concat(components(path), '/')
end

local function targetPath(root: string, key: string, preservePaths: boolean): string
  checkExtractable(key)
  if not preservePaths then
    -- A normalized path's file name: its last component, a trailing separator aside.
    local name = string.match(key, '([^/]*)/?$') :: string
    if name == '' then unsafePath(key) end

    return join(root, name)
  end

  -- A key with no component to drop or to end on goes below the root as it is.
  local first = string.byte(key, 1)
  if
    first ~= 46
    and first ~= 47
    and string.byte(key, -1) ~= 47
    and not string.find(key, '/[./]')
  then
    return join(root, key)
  end

  local target = root
  for component in string.gmatch(key .. '/', '([^/]*)/') do
    if component == '.' then continue end

    target = join(target, component)
  end

  return target
end

local function exists(path: string): boolean
  return fs.exists(path) == true
end

local function createFolder(path: string)
  local created, message = fs.mkdir(path, { recursive = true })
  if not created then ioError(util.hostMessage(message, path)) end
end

local function removeQuietly(path: string)
  fs.remove(path)
end

local Salt = math.random(0, 0x7fffffff)
local counter = 0

--- A temporary file's path beside `target`, to write and then rename over it.
local function temporaryBeside(folder: string, target: string): string
  counter += 1
  return join(folder, `.{fileName(target) or 'archive'}.dream-archivetool-{Salt}-{counter}`)
end

local function rename(from: string, to: string)
  local ok, message = fs.rename(from, to)
  if ok then return end

  removeQuietly(from)
  ioError(util.hostMessage(message, from))
end

type Listed = {
  index: number,
  raw: string,
  key: string,
  size: number?,
  compressedSize: number?,
}

--- An entry's name as the archive stores it, or nil for one with only a hash.
local function rawPath(backend: Backend, index: number): string?
  local path = backend.path(index)
  if backend.format == 'ba2' and path == '' then return nil end

  return path
end

local function namedCount(backend: Backend): number
  if backend.format == 'bsaTes3' then return backend.count end

  return #[for index in range(1, backend.count) if rawPath(backend, index) => index]
end

local function chunkSizes(backend: Backend, index: number): (number, number?)
  local size, packed = 0, 0
  for _, chunk in (backend.ba2 :: types.BA2Facts).chunkList(index) do
    size += chunk.size
    packed += chunk.packedSize
  end

  return size, if packed > 0 then packed else nil
end

local function listEntries(backend: Backend): { Listed }
  local listed = {}
  for index = 1, backend.count do
    local raw = rawPath(backend, index)
    if not raw then continue end

    local row: Listed = { index = index, raw = raw :: string, key = lookupKey(raw :: string) }
    if backend.format == 'ba2' then
      row.size, row.compressedSize = chunkSizes(backend, index)
    else
      row.size = (backend.size(index))
      if backend.compressed(index) then row.compressedSize = backend.storedSize(index) end
    end

    table.insert(listed, row)
  end

  return listed
end

--- A member's bytes by its normalized path.
local function readMember(archive: Archive, key: string): string
  -- readFileRequired throws the crate's reason a member can't be read.
  local ok, result = pcall(archive.readFileRequired, archive, key)
  if not ok then readError(key, result) end

  return result :: string
end

local function tes4Facts(backend: Backend): types.Tes4Facts?
  return if backend.format == 'bsaTes4' then backend.tes4 else nil
end

local function hasFlag(flags: number, flag: number): boolean return bit32.band(flags, flag) ~= 0 end

local function recoverableNames(flags: number): boolean
  return hasFlag(flags, Tes4Flags.directoryStrings) and hasFlag(flags, Tes4Flags.fileStrings)
    or hasFlag(flags, Tes4Flags.embeddedFileNames)
end

local function rewriteBlocker(backend: Backend): string?
  if namedCount(backend) ~= backend.count then return UnnameableBlocker end

  local facts = tes4Facts(backend)
  if facts and not recoverableNames(facts.flags) then return HashOnlyBlocker end

  if facts and bit32.band(facts.flags, bit32.bnot(RewritableTes4Flags)) ~= 0 then
    return UnsupportedFlagsBlocker
  end

  if backend.ba2 and backend.ba2.format == 'gnmf' then return GnmfBlocker end

  return nil
end

local function nameMode(flags: number): string
  local strings = hasFlag(flags, Tes4Flags.directoryStrings)
    and hasFlag(flags, Tes4Flags.fileStrings)
  local embedded = hasFlag(flags, Tes4Flags.embeddedFileNames)
  if strings and embedded then return 'stringsAndEmbedded' end

  if strings then return 'strings' end

  return if embedded then 'embedded' else 'hashOnly'
end

local function infoOf(archive: Archive, label: string): ArchiveInfo
  local backend = backendOf(archive)
  local named = namedCount(backend)
  local blocker = rewriteBlocker(backend)
  local info: ArchiveInfo = {
    path = label,
    format = backend.format,
    fileCount = backend.count,
    namedEntryCount = named,
    hasUnnameableEntries = named ~= backend.count,
    rewritable = blocker == nil,
    rewriteBlocker = blocker,
  }

  local facts = tes4Facts(backend)
  if facts then
    local flags = [for {name, bit} in NamedTes4Flags if hasFlag(facts.flags, bit) => name]
    info.tes4 = {
      version = `v{facts.version}`,
      archiveTypes = `ArchiveTypes({facts.archiveTypes})`,
      archiveTypesBits = facts.archiveTypes,
      archiveFlags = flags,
      archiveFlagsBits = facts.flags,
      unsupportedArchiveFlagsBits = bit32.band(facts.flags, bit32.bnot(RewritableTes4Flags)),
      nameMode = nameMode(facts.flags),
    }
  end

  local ba2Facts = backend.ba2
  if ba2Facts then
    local described = archive:info()
    info.ba2 = {
      version = `v{described.version}`,
      payloadFormat = described.format,
      compressionFormat = described.compression,
      strings = described.strings,
    }
  end

  return info
end

local function info(...: unknown): ArchiveInfo
  argumentCount('info', 1, select('#', ...))
  return infoOf(openSource(checkSource('info', (...), 1)))
end

--- The archive's named entries in archive order, as the crate's `ArchiveTool::list` gives them.
local function list(...: unknown): { ArchiveEntry }
  argumentCount('list', 1, select('#', ...))
  local archive = openSource(checkSource('list', (...), 1))
  return [for {key, size, compressedSize} in listEntries(backendOf(archive)) => {
    path = util.lossy(key),
    pathBytesHex = bytes.toHex(key),
    size = size,
    compressedSize = compressedSize,
  }]
end

local function verify(...: unknown): VerifyReport
  argumentCount('verify', 1, select('#', ...))
  local source, options = ...
  local checked = checkSource('verify', source, 1)
  local readPayloads = flagOption('verify', options, 'readPayloads')
  local archive, label = openSource(checked)
  local described = infoOf(archive, label)
  local entries = listEntries(backendOf(archive))
  local seen: { [string]: string } = {}
  local duplicates: { PathIssue }, unsafe: { PathIssue } = {}, {}
  for _, entry in entries do
    local earlier = seen[entry.key]
    if earlier then
      table.insert(duplicates, {
        path = util.lossy(entry.key),
        pathBytesHex = bytes.toHex(entry.key),
        rawPathBytesHex = bytes.toHex(entry.raw),
        collidingRawPathBytesHex = bytes.toHex(earlier),
      })
    else
      seen[entry.key] = entry.raw
    end

    if not extractable(entry.raw) or not extractable(entry.key) then
      table.insert(unsafe, {
        path = util.lossy(entry.key),
        pathBytesHex = bytes.toHex(entry.key),
        rawPathBytesHex = bytes.toHex(entry.raw),
      })
    end
  end

  local payloadsRead: number? = nil
  if readPayloads and #duplicates == 0 then
    for _, entry in entries do
      readMember(archive, entry.key)
    end

    payloadsRead = #entries
  end

  local warnings = {}
  if described.hasUnnameableEntries then
    table.insert(warnings, 'archive contains entries without recoverable path names')
  end

  if #duplicates > 0 then
    table.insert(warnings, 'archive contains duplicate normalized paths')
    if readPayloads then
      table.insert(
        warnings,
        'payload read verification skipped because duplicate normalized paths prevent per-entry coverage'
      )
    end
  end

  if #unsafe > 0 then
    table.insert(warnings, 'archive contains paths unsafe to extract directly')
  end

  return {
    path = described.path,
    format = described.format,
    fileCount = described.fileCount,
    namedEntryCount = described.namedEntryCount,
    unnameableEntries = described.fileCount - described.namedEntryCount,
    rewritable = described.rewritable,
    rewriteBlocker = described.rewriteBlocker,
    duplicateNormalizedPaths = duplicates,
    unsafePaths = unsafe,
    payloadsRead = payloadsRead,
    warnings = warnings,
  }
end

type Compared = { raw: string, size: number?, compressedSize: number?, fingerprint: integer? }

local function rejectUnnameable(archive: Archive, label: string)
  local backend = backendOf(archive)
  if namedCount(backend) == backend.count then return end

  archiveError(
    `archive '{label}' contains entries without recoverable paths; refusing to diff it lossy`
  )
end

local function diffEntries(
  archive: Archive,
  fingerprints: boolean
): ({ [string]: Compared }, { string })
  local backend = backendOf(archive)
  local compared: { [string]: Compared } = {}
  local keys = {}
  for _, entry in listEntries(backend) do
    local fingerprint: integer? = nil
    if fingerprints then fingerprint = bytes.fnv1a64(readMember(archive, entry.key)) end

    local earlier = compared[entry.key]
    if earlier then
      archiveError(
        `archive contains duplicate normalized path: {util.lossy(entry.key)} (first raw path hex: {bytes.toHex(
          earlier.raw
        )}, duplicate raw path hex: {bytes.toHex(entry.raw)})`
      )
    end

    compared[entry.key] = {
      raw = entry.raw,
      size = entry.size,
      compressedSize = entry.compressedSize,
      fingerprint = fingerprint,
    }
    table.insert(keys, entry.key)
  end

  table.sort(keys)
  return compared, keys
end

local function stateOf({size, compressedSize, fingerprint}: Compared): DiffState
  return { size = size, compressedSize = compressedSize, payloadFingerprint = fingerprint }
end

local function diffEntry(key: string, {size, compressedSize, fingerprint}: Compared): DiffEntry
  return {
    path = util.lossy(key),
    pathBytesHex = bytes.toHex(key),
    size = size,
    compressedSize = compressedSize,
    payloadFingerprint = fingerprint,
  }
end

local function diff(...: unknown): DiffReport
  argumentCount('diff', 2, select('#', ...))
  local oldSource, newSource, options = ...
  local oldChecked = checkSource('diff', oldSource, 1)
  local newChecked = checkSource('diff', newSource, 2)
  local fingerprints = flagOption('diff', options, 'fingerprintPayloads')
  local oldArchive, oldLabel = openSource(oldChecked)
  local newArchive, newLabel = openSource(newChecked)
  rejectUnnameable(oldArchive, oldLabel)
  rejectUnnameable(newArchive, newLabel)
  local oldEntries, oldKeys = diffEntries(oldArchive, fingerprints)
  local newEntries, newKeys = diffEntries(newArchive, fingerprints)
  local removed, changed = {}, {}
  local unchanged = 0
  for _, key in oldKeys do
    local before, after = oldEntries[key], newEntries[key]
    if not after then
      table.insert(removed, diffEntry(key, before))
      continue
    end

    if
      before.size == after.size
      and before.compressedSize == after.compressedSize
      and before.fingerprint == after.fingerprint
    then
      unchanged += 1
      continue
    end

    table.insert(changed, {
      path = util.lossy(key),
      pathBytesHex = bytes.toHex(key),
      old = stateOf(before),
      new = stateOf(after),
    })
  end

  local added = [for key in newKeys if not oldEntries[key] => diffEntry(key, newEntries[key])]
  return {
    old = oldLabel,
    new = newLabel,
    comparison = if fingerprints then 'payloadFingerprint' else 'metadataOnly',
    fingerprintPayloads = fingerprints,
    added = added,
    removed = removed,
    changed = changed,
    unchanged = unchanged,
  }
end

type Target = { key: string, path: string }

--- The archive path an entry argument names: a string as it is, an entry by its path.
local function entryPath(value: unknown, context: string): string
  if archives.isEntry(value) then
    local entry = value :: types.Entry
    local path = entry.path
    if not path then
      fail(
        `{context}: entry {entry.index} has no path; hash-only entries are not extraction targets`
      )
    end

    return path :: string
  end

  if type(value) == 'string' then return value :: string end

  return fail(`{context}: expected an archive path or an entry handle, got {type(value)}`)
end

local function entryList(value: unknown, context: string): { string }
  if type(value) ~= 'table' then
    fail(
      `{context}: entries must be an array of archive paths or an entries view, got {type(value)}`
    )
  end

  denseLength(value :: { [unknown]: unknown }, `{context}.entries`)
  local paths: { string } = {}
  for position, item in value :: { unknown } do
    table.insert(paths, entryPath(item, `{context}.entries[{position}]`))
  end

  return paths
end

local function hexList(value: unknown, context: string): { string }
  if type(value) ~= 'table' then fail(`Lua stack index 2: expected table, got {type(value)}`) end

  denseLength(value :: { [unknown]: unknown }, `{context}.entries`)
  local paths: { string } = {}
  for position, item in value :: { unknown } do
    local path = `{context}.entries[{position}]`
    if type(item) ~= 'string' or not utf8.len(item :: string) then
      fail(`{path}: expected a UTF-8 pathBytesHex string, got {type(item)}`)
    end

    table.insert(paths, decodeHex(item :: string, path))
  end

  return paths
end

local function hexEntry(value: string, context: string): string
  return decodeHex(value, context)
end

--- Whether no two of `targets` are one path.
local function distinctTargets(targets: { Target }): boolean
  local seen: { [string]: boolean } = {}
  for _, target in targets do
    local identity = pathIdentity(target.path)
    if seen[identity] then return false end

    seen[identity] = true
  end

  return true
end

local function rejectDuplicateTargets(targets: { Target })
  if distinctTargets(targets) then return end

  -- The crate names the first of the two that sorting by `Path`'s order puts side by side.
  local paths = [for target in targets => target.path]
  table.sort(paths, pathLess)
  for index = 2, #paths do
    if samePath(paths[index - 1], paths[index]) then
      archiveError(
        `duplicate extraction target after normalization: {util.lossy(paths[index - 1])}`
      )
    end
  end
end

local function targetsFromPaths(root: string, paths: { string }, preservePaths: boolean): { Target }
  local targets: { Target } = {}
  for _, path in paths do
    local key = safeKey(path)
    table.insert(targets, { key = key, path = targetPath(root, key, preservePaths) })
  end

  rejectDuplicateTargets(targets)
  return targets
end

local function requireTargets(archive: Archive, targets: { Target })
  for _, target in targets do
    if not archive:contains(target.key) then entryNotFound(target.key) end
  end
end

local function rejectExisting(targets: { Target }, overwrite: Overwrite)
  if overwrite ~= 'fail' then return end

  for _, target in targets do
    if exists(target.path) then targetExists(target.path) end
  end
end

--- A folder to make, with the components its place among the others is decided by.
type TargetFolder = { path: string, parts: { string } }

local function prepareFolders(targets: { Target })
  local folders: { [string]: boolean } = {}
  local sorted: { TargetFolder } = {}
  for _, target in targets do
    local folder = parentDirectory(target.path)
    if folders[folder] then continue end

    folders[folder] = true
    table.insert(sorted, { path = folder, parts = components(folder) })
  end

  table.sort(
    sorted,
    function(left: TargetFolder, right: TargetFolder): boolean
      return componentsLess(left.parts, right.parts)
    end
  )

  for _, folder in sorted do
    createFolder(folder.path)
  end
end

--- Writes one member to `target` through a temporary file beside it, as `overwrite` says.
--- Returns the files extracted and skipped.
local function writeTarget(
  archive: Archive,
  target: Target,
  overwrite: Overwrite,
  createParent: boolean
): (number, number)
  if exists(target.path) then
    if overwrite == 'fail' then targetExists(target.path) end

    if overwrite == 'skip' then return 0, 1 end
  end

  local folder = parentDirectory(target.path)
  if createParent then createFolder(folder) end

  local data = readMember(archive, target.key)
  local temporary = temporaryBeside(folder, target.path)
  local written, problem = fs.writeFile(temporary, data)
  if not written then
    removeQuietly(temporary)
    ioError(util.hostMessage(problem, temporary))
  end

  -- `@dream/fs` has no rename that refuses an existing target, so the check is made again just
  -- before the rename instead.
  if overwrite ~= 'overwrite' and exists(target.path) then
    removeQuietly(temporary)
    if overwrite == 'fail' then targetExists(target.path) end

    return 0, 1
  end

  rename(temporary, target.path)
  return 1, 0
end

local function extractTargets(archive: Archive, targets: { Target }, options: Extraction): Summary
  rejectExisting(targets, options.overwrite)
  prepareFolders(targets)
  local summary: Summary = { extracted = 0, skipped = 0 }
  for _, target in targets do
    local extracted, skipped = writeTarget(archive, target, options.overwrite, false)
    summary.extracted += extracted
    summary.skipped += skipped
  end

  return summary
end

type PlanAction = 'extract' | 'overwrite' | 'skip' | 'conflict'

--- A row for each target: what extracting it would do, given whether something is there already.
local function planRows(targets: { Target }, overwrite: Overwrite): { ExtractPlanRow }
  local absent: PlanAction = 'extract'
  local present: PlanAction = if overwrite == 'fail'
    then 'conflict'
    elseif overwrite == 'overwrite' then 'overwrite'
    else 'skip'

  return [for {key, path} in targets => {
    action = if exists(path) then present else absent,
    path = util.lossy(key),
    pathBytesHex = bytes.toHex(key),
    target = util.lossy(path),
  }]
end

local function extractOne(
  archive: Archive,
  path: string,
  {output, overwrite, preservePaths}: Extraction
): Summary
  local key = safeKey(path)
  if not archive:contains(key) then entryNotFound(key) end

  local target = { key = key, path = targetPath(output, key, preservePaths) }
  local extracted, skipped = writeTarget(archive, target, overwrite, true)
  return { extracted = extracted, skipped = skipped }
end

local function extract(...: unknown): Summary
  argumentCount('extract', 2, select('#', ...))
  local source, entry, options = ...
  local checked = checkSource('extract', source, 1)
  local path = entryPath(entry, 'extract')
  local chosen = extractOptions('extract', options)
  return extractOne((openSource(checked)), path, chosen)
end

local function byPathHex(what: string): (...unknown) -> Summary
  return function(...: unknown): Summary
    argumentCount(what, 2, select('#', ...))
    local source, entry, options = ...
    local checked = checkSource(what, source, 1)
    local text = hostPath(what, entry, 2)
    local path = hexEntry(text, what)
    local chosen = extractOptions(what, options)
    return extractOne((openSource(checked)), path, chosen)
  end
end

--- A member's bytes, as the crate's `ArchiveTool::read_entry_by_path_bytes` reads them: the
--- archive is opened, and then the path is normalized for lookup. Nothing is written, so the
--- path need not be one extraction would accept.
local function readEntry(...: unknown): string
  argumentCount('readEntry', 2, select('#', ...))
  local source, entry = ...
  local checked = checkSource('readEntry', source, 1)
  local path = entryPath(entry, 'readEntry')
  local archive = openSource(checked)
  return readMember(archive, lookupKey(path))
end

local function readEntryByPathHex(...: unknown): string
  argumentCount('readEntryByPathHex', 2, select('#', ...))
  local source, entry = ...
  local checked = checkSource('readEntryByPathHex', source, 1)
  local text = hostPath('readEntryByPathHex', entry, 2)
  local path = hexEntry(text, 'readEntryByPathHex')
  local archive = openSource(checked)
  return readMember(archive, lookupKey(path))
end

local function extractManyWith(
  what: string,
  read: (unknown, string) -> { string }
): (...unknown) -> Summary
  return function(...: unknown): Summary
    argumentCount(what, 2, select('#', ...))
    local source, entries, options = ...
    local checked = checkSource(what, source, 1)
    local paths = read(entries, what)
    local chosen = extractOptions(what, options)
    local archive = openSource(checked)
    local targets = targetsFromPaths(chosen.output, paths, chosen.preservePaths)
    requireTargets(archive, targets)
    return extractTargets(archive, targets, chosen)
  end
end

local function planExtractWith(
  what: string,
  read: (unknown, string) -> { string }
): (...unknown) -> ExtractPlan
  return function(...: unknown): ExtractPlan
    argumentCount(what, 2, select('#', ...))
    local source, entries, options = ...
    local checked = checkSource(what, source, 1)
    local paths = read(entries, what)
    local chosen = extractOptions(what, options)
    local archive, label = openSource(checked)
    local targets = targetsFromPaths(chosen.output, paths, chosen.preservePaths)
    requireTargets(archive, targets)
    return {
      operation = 'extract',
      archive = label,
      output = chosen.output,
      entries = planRows(targets, chosen.overwrite),
    }
  end
end

--- Every named entry's target, refused when any entry has no name or an unsafe one.
local function allTargets(archive: Archive, root: string): { Target }
  local backend = backendOf(archive)
  local entries = listEntries(backend)
  if #entries ~= backend.count then
    archiveError 'archive contains entries without recoverable paths; refusing to extract it lossy'
  end

  local targets: { Target } = {}
  for _, entry in entries do
    checkExtractable(entry.raw)
    table.insert(targets, { key = entry.key, path = targetPath(root, entry.key, true) })
  end

  rejectDuplicateTargets(targets)
  return targets
end

local function extractAll(...: unknown): Summary
  argumentCount('extractAll', 1, select('#', ...))
  local source, options = ...
  local checked = checkSource('extractAll', source, 1)
  local chosen = extractAllOptions('extractAll', options)
  local archive = openSource(checked)
  return extractTargets(archive, allTargets(archive, chosen.output), chosen)
end

local function planExtractAll(...: unknown): ExtractPlan
  argumentCount('planExtractAll', 1, select('#', ...))
  local source, options = ...
  local checked = checkSource('planExtractAll', source, 1)
  local chosen = extractAllOptions('planExtractAll', options)
  local archive, label = openSource(checked)
  return {
    operation = 'extractAll',
    archive = label,
    output = chosen.output,
    entries = planRows(allTargets(archive, chosen.output), chosen.overwrite),
  }
end

type Inputs = { [string]: string }

local function canonical(path: string): string?
  local result = fs.canonicalize(path)
  return result
end

--- Whether `path`, in the folder whose canonical path is `folder`, is a symbolic link: it
--- resolves somewhere other than its own name there. A path that doesn't resolve isn't one.
local function isLink(path: string, folder: string?): boolean
  local name = fileName(path)
  local real = canonical(path)
  if not name or not folder or not real then return false end

  return real ~= join(folder :: string, name :: string)
end

local function rejectLink(path: string, folder: string?, followSymlinks: boolean)
  if followSymlinks or not isLink(path, folder) then return end

  fail(`refusing to follow symlink input path: {util.lossy(path)}; set followSymlinks to opt in`)
end

--- The archive path of a host path relative to an input: its components joined with `/`,
--- normalized, and checked for extraction.
local function archiveKey(relative: string): string
  local key = util.lookupKey(relative)
  checkExtractable(key)
  return key
end

local function insertInput(inputs: Inputs, key: string, source: string)
  local previous = inputs[key]
  if previous then
    archiveError(
      `duplicate archive path after normalization: {util.lossy(key)} (first source: {util.lossy(
        previous
      )}, duplicate source: {util.lossy(source)})`
    )
  end

  inputs[key] = source
end

type Stat = { isDir: boolean, isFile: boolean, size: number }

local function stat(path: string): Stat
  local result, message = fs.stat(path)
  if message then ioError(util.hostMessage(message, path)) end

  if not result then ioError 'No such file or directory (os error 2)' end

  return result :: Stat
end

type Folder = { path: string, relative: string, real: string? }

--- Every file below `folder`, depth first in name order, as walking it finds them: a folder's
--- files and folders are taken in turn, each folder's contents before the next name.
local function walk(inputs: Inputs, folder: Folder, ancestors: { Folder }, followSymlinks: boolean)
  local names, message = fs.list(folder.path)
  if not names then archiveError(util.hostMessage(message, folder.path)) end

  -- Gather child metadata before processing inputs, preserving traversal error precedence.
  local found, problem = fs.walk(folder.path, { followLinks = true, metadata = true, maxDepth = 1 })
  if problem then archiveError(util.walkMessage(problem)) end

  local rows = assert(found)
  local ordered: { number } = {}
  for index in rows.paths do table.insert(ordered, index) end

  table.sort(ordered, function(first: number, second: number): boolean
    return rows.paths[first] < rows.paths[second]
  end)

  for _, index in ordered do
    local name = rows.paths[index]
    local path = join(folder.path, name)
    rejectLink(path, folder.real, followSymlinks)
    local relative = if folder.relative == '' then name else `{folder.relative}/{name}`
    if rows.kinds[index] ~= 'dir' then
      insertInput(inputs, archiveKey(relative), path)
      continue
    end

    local child: Folder = { path = path, relative = relative, real = canonical(path) }
    for _, ancestor in ancestors do
      if child.real and child.real == ancestor.real then
        archiveError(
          `File system loop found: {util.lossy(path)} points to an ancestor {util.lossy(
            ancestor.path
          )}`
        )
      end
    end

    table.insert(ancestors, child)
    walk(inputs, child, ancestors, followSymlinks)
    table.remove(ancestors)
  end
end

--- The files `input` names, a file or a folder of them, added to `inputs` by archive path.
local function collectInto(inputs: Inputs, input: string, followSymlinks: boolean)
  rejectLink(input, canonical(parentDirectory(input)), followSymlinks)
  local found = stat(input)
  if not found.isDir then
    local name = fileName(input)
    if not name then unsafePath(input) end

    insertInput(inputs, archiveKey(name :: string), input)
    return
  end

  local root: Folder = { path = input, relative = '', real = canonical(input) }
  walk(inputs, root, { root }, followSymlinks)
end

local function sortedKeys(inputs: Inputs): { string }
  local keys = {}
  for key in inputs do
    table.insert(keys, key)
  end

  table.sort(keys)
  return keys
end

--- A normalized path's extension: what follows the last `.` of its file name, if anything
--- comes before it.
local function extensionOf(key: string): string?
  local name = string.match(key, '([^/]*)/?$') :: string
  return string.match(name, '^.+%.([^.]*)$')
end

local function checkBa2Paths(keys: { string }, kind: string)
  if kind == 'gnrl' then return end

  local extension = if kind == 'dx10' then 'dds' else 'gnf'
  local label = if kind == 'dx10'
    then 'DX10 BA2 archives can only contain .dds files'
    else 'GNMF BA2 archives can only contain .gnf files'
  for _, key in keys do
    if extensionOf(key) ~= extension then archiveError(`{label}: {util.lossy(key)}`) end
  end
end

local function planRow(
  action: 'add' | 'replace' | 'preserve',
  key: string,
  source: string?
): ArchivePlanRow
  return {
    action = action,
    source = if source then util.lossy(source :: string) else nil,
    path = util.lossy(key),
    pathBytesHex = bytes.toHex(key),
    size = if source then stat(source :: string).size else nil,
  }
end

--- What the tool asks of the four builders; each has the members its format uses.
type Builder = {
  addFile: (self: Builder, archivePath: string, source: string) -> (),
  addDdsFile: (self: Builder, archivePath: string, source: string) -> (),
  addArchiveEntry: (self: Builder, archivePath: string, from: Archive, id: number) -> (),
  addArchiveEntryWithCompression: (
    self: Builder,
    archivePath: string,
    from: Archive,
    id: number,
    mode: string
  ) -> (),
  writePath: (self: Builder, path: string) -> (),
}

--- Runs a builder call, its failure an archive error.
local function building(body: () -> ())
  -- The builders throw the crate's reasons a member can't be added or an archive written.
  local ok, problem = pcall(body)
  if not ok then archiveError(util.hostMessage(problem)) end
end

--- Writes the archive `build` makes to `output` through a temporary file in its folder,
--- renamed into place once the whole archive is written.
local function writeArchive(output: string, build: () -> Builder)
  local folder = parentDirectory(output)
  if not stat(folder).isDir then ioError 'Not a directory (os error 20)' end

  local temporary = temporaryBeside(folder, output)
  -- build and Builder.writePath throw; remove the partial temporary, then rethrow every failure.
  local ok, problem = pcall(function()
    local builder = build()
    building(function() builder:writePath(temporary) end)
  end)

  if not ok then
    removeQuietly(temporary)
    fail(tostring(problem))
  end

  rename(temporary, output)
end

local function newBuilder(options: Creation, keys: { string }, inputs: Inputs): Builder
  local dds = options.format == 'ba2' and options.ba2Kind == 'dx10'
  local builder: Builder
  if options.format == 'bsaTes3' then
    builder = (tes3.Builder.new() :: unknown) :: Builder
  elseif options.format == 'bsaTes4' then
    local made = tes4.Builder.new()
    made:setVersion(options.tes4Version)
    if options.compress then made:setCompressed(true) end

    builder = (made :: unknown) :: Builder
  else
    local made = if dds then ba2.Dx10Builder.new() else ba2.Builder.new()
    made:setVersion(options.ba2Version)
    if options.compress then made:setCompression 'zip' end

    builder = (made :: unknown) :: Builder
  end

  for _, key in keys do
    local source = inputs[key]
    building(function()
      if dds then
        builder:addDdsFile(key, source)
      else
        builder:addFile(key, source)
      end
    end)
  end

  return builder
end

local function collectCreate(what: string, ...: unknown): (string, Creation, Inputs, { string })
  argumentCount(what, 2, select('#', ...))
  local outputArgument, inputArgument, options = ...
  local output = hostPath(what, outputArgument, 1)
  local input = hostPath(what, inputArgument, 2)
  local chosen = createOptions(what, options)
  if chosen.format == 'ba2' and chosen.ba2Kind == 'gnmf' then archiveError(GnmfBlocker) end

  local inputs: Inputs = {}
  collectInto(inputs, input, chosen.followSymlinks)
  local keys = sortedKeys(inputs)
  if chosen.format == 'ba2' then checkBa2Paths(keys, chosen.ba2Kind) end

  return output, chosen, inputs, keys
end

local function create(...: unknown): { files: number }
  local output, chosen, inputs, keys = collectCreate('create', ...)
  writeArchive(output, function(): Builder return newBuilder(chosen, keys, inputs) end)
  return { files = #keys }
end

local function planCreate(...: unknown): CreatePlan
  local output, chosen, inputs, keys = collectCreate('planCreate', ...)
  local rows = [for key in keys => planRow('add', key, inputs[key])]
  return {
    operation = 'create',
    format = chosen.format,
    output = output,
    files = #rows,
    entries = rows,
  }
end

--- A path as the crate compares two: canonical when it exists, else its canonical folder joined
--- with its name.
local function comparable(path: string): string
  if exists(path) then
    local result, message = fs.canonicalize(path)
    if not result then ioError(util.hostMessage(message, path)) end

    return result :: string
  end

  local parent = parentDirectory(path)
  local folder, message = fs.canonicalize(parent)
  if not folder then ioError(util.hostMessage(message, parent)) end

  local name = fileName(path)
  return if name then join(folder :: string, name :: string) else folder :: string
end

type Rewrite = {
  archive: Archive,
  label: string,
  output: string,
  inputs: Inputs,
  keys: { string },
}

local function prepareAdd(what: string, ...: unknown): Rewrite
  argumentCount(what, 2, select('#', ...))
  local archiveArgument, options = ...
  local path = hostPath(what, archiveArgument, 1)
  local chosen = addOptions(what, options)
  if chosen.output and comparable(path) == comparable(chosen.output :: string) then
    archiveError 'explicit output archive path must differ from input archive path; omit output to replace the input archive'
  end

  local archive, label = openSource(path)
  local backend = backendOf(archive)
  local blocker = rewriteBlocker(backend)
  if blocker then archiveError(blocker) end

  local inputs: Inputs = {}
  for _, input in chosen.inputs do
    local collected: Inputs = {}
    collectInto(collected, input, chosen.followSymlinks)
    for _, key in sortedKeys(collected) do
      insertInput(inputs, key, collected[key])
    end
  end

  local keys = sortedKeys(inputs)
  if backend.ba2 then checkBa2Paths(keys, backend.ba2.format) end

  return {
    archive = archive,
    label = label,
    output = chosen.output or path,
    inputs = inputs,
    keys = keys,
  }
end

--- The archive's named entries by normalized path, in archive order, refusing duplicates.
local function existingEntries(backend: Backend): { { index: number, key: string } }
  local raws: { [string]: string } = {}
  local entries = {}
  for index = 1, backend.count do
    local raw = rawPath(backend, index)
    if not raw then continue end

    local key = util.lookupKey(raw :: string)
    local earlier = raws[key]
    if earlier then
      archiveError(
        `archive contains duplicate normalized path: {util.lossy(key)} (first raw path hex: {bytes.toHex(
          earlier
        )}, duplicate raw path hex: {bytes.toHex(raw :: string)})`
      )
    end

    raws[key] = raw :: string
    table.insert(entries, { index = index, key = key })
  end

  return entries
end

local function planAdd(...: unknown): AddPlan
  local rewrite = prepareAdd('planAdd', ...)
  local backend = backendOf(rewrite.archive)
  local existing: { [string]: boolean } = {}
  local existingKeys = {}
  for _, entry in existingEntries(backend) do
    existing[entry.key] = true
    table.insert(existingKeys, entry.key)
  end

  table.sort(existingKeys)
  local rows = {}
  local added, replaced, preserved = 0, 0, 0
  for _, key in existingKeys do
    if rewrite.inputs[key] then
      replaced += 1
      continue
    end

    preserved += 1
    table.insert(rows, planRow('preserve', key, nil))
  end

  for _, key in rewrite.keys do
    local action: 'add' | 'replace' = 'replace'
    if not existing[key] then
      added += 1
      action = 'add'
    end

    table.insert(rows, planRow(action, key, rewrite.inputs[key]))
  end

  return {
    operation = 'add',
    archive = rewrite.label,
    output = rewrite.output,
    format = backend.format,
    files = #rows,
    added = added,
    replaced = replaced,
    preserved = preserved,
    entries = rows,
  }
end

--- The compression a rewrite gives what it writes afresh: the archive's own, when anything in
--- it is compressed or it names LZ4, since textures are copied only under their own.
local function rewriteCompression(backend: Backend, facts: types.BA2Facts): string?
  if facts.compression == 'lz4' then return 'lz4' end

  if any[for index in range(1, backend.count) => backend.compressed(index)] then
    return facts.compression
  end

  return nil
end

--- A builder in the archive's own format and settings, with what it keeps first, in archive
--- order, and then the inputs. Returns it and how many members it has.
local function rewriteBuilder(rewrite: Rewrite): (Builder, number)
  local archive = rewrite.archive
  local backend = backendOf(archive)
  local facts, ba2Facts = tes4Facts(backend), backend.ba2
  local builder: Builder
  local dds = false
  if facts then
    local unsupported = bit32.band(facts.flags, bit32.bnot(RewritableTes4Flags))
    if unsupported ~= 0 then
      archiveError(
        `TES4 archive uses header flag bits this tool cannot preserve ({string.format(
          '0x%08x',
          unsupported
        )}); refusing to rewrite it lossy`
      )
    end

    local made = tes4.Builder.new()
    made:setVersion(facts.version)
    made:setArchiveTypes(facts.archiveTypes)
    made:setCompressed(hasFlag(facts.flags, Tes4Flags.compressed))
    made:setNameMode(nameMode(facts.flags))
    builder = (made :: unknown) :: Builder
  elseif ba2Facts then
    if ba2Facts.format == 'gnmf' then archiveError(GnmfBlocker) end

    dds = ba2Facts.format == 'dx10'
    local made = if dds then ba2.Dx10Builder.new() else ba2.Builder.new()
    made:setVersion(ba2Facts.version)
    made:setCompression(rewriteCompression(backend, ba2Facts))
    builder = (made :: unknown) :: Builder
  else
    builder = (tes3.Builder.new() :: unknown) :: Builder
  end

  local general = ba2Facts ~= nil and not dds
  local count = 0
  for _, entry in existingEntries(backend) do
    if rewrite.inputs[entry.key] then continue end

    count += 1
    building(function()
      if not general then
        builder:addArchiveEntry(entry.key, archive, entry.index)
        return
      end

      local mode = if backend.compressed(entry.index) then 'compress' else 'store'
      builder:addArchiveEntryWithCompression(entry.key, archive, entry.index, mode)
    end)
  end

  for _, key in rewrite.keys do
    local source = rewrite.inputs[key]
    building(function()
      if dds then
        builder:addDdsFile(key, source)
      else
        builder:addFile(key, source)
      end
    end)
  end

  return builder, count + #rewrite.keys
end

local function add(...: unknown): { files: number }
  local rewrite = prepareAdd('add', ...)
  local files = 0
  writeArchive(rewrite.output, function(): Builder
    local builder, count = rewriteBuilder(rewrite)
    files = count
    return builder
  end)
  return { files = files }
end

return table.freeze {
  version = Version,
  info = info :: (archive: Source) -> ArchiveInfo,
  list = list :: (archive: Source) -> { ArchiveEntry },
  readEntry = readEntry :: (archive: Source, entry: EntryArgument) -> string,
  readEntryByPathHex = readEntryByPathHex :: (archive: Source, pathBytesHex: string) -> string,
  verify = verify :: (archive: Source, options: { readPayloads: boolean? }?) -> VerifyReport,
  diff = diff :: (
    old: Source,
    new: Source,
    options: { fingerprintPayloads: boolean? }?
  ) -> DiffReport,
  extract = extract :: (archive: Source, entry: EntryArgument, options: ExtractOptions?) -> Summary,
  extractByPathHex = byPathHex 'extractByPathHex' :: (
    archive: Source,
    pathBytesHex: string,
    options: ExtractOptions?
  ) -> Summary,
  extractHex = byPathHex 'extractHex' :: (
    archive: Source,
    pathBytesHex: string,
    options: ExtractOptions?
  ) -> Summary,
  extractMany = extractManyWith('extractMany', entryList) :: (
    archive: Source,
    entries: EntryList,
    options: ExtractOptions?
  ) -> Summary,
  extractManyByPathHex = extractManyWith('extractManyByPathHex', hexList) :: (
    archive: Source,
    pathBytesHex: { string },
    options: ExtractOptions?
  ) -> Summary,
  planExtract = planExtractWith('planExtract', entryList) :: (
    archive: Source,
    entries: EntryList,
    options: ExtractOptions?
  ) -> ExtractPlan,
  planExtractByPathHex = planExtractWith('planExtractByPathHex', hexList) :: (
    archive: Source,
    pathBytesHex: { string },
    options: ExtractOptions?
  ) -> ExtractPlan,
  extractAll = extractAll :: (archive: Source, options: ExtractAllOptions?) -> Summary,
  planExtractAll = planExtractAll :: (archive: Source, options: ExtractAllOptions?) -> ExtractPlan,
  create = create :: (output: string, input: string, options: CreateOptions?) -> { files: number },
  planCreate = planCreate :: (output: string, input: string, options: CreateOptions?) -> CreatePlan,
  add = add :: (archive: string, options: AddOptions) -> { files: number },
  planAdd = planAdd :: (archive: string, options: AddOptions) -> AddPlan,
}
