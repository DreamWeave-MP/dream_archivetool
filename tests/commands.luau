--!strict
--!native
-- `@dream-archivetool/commands`: each subcommand's request in, and its result table out. The
-- scenarios are dream_archivetool's own, from its command line's tests, held to what a command
-- returns and leaves on disk: nothing here looks at printed text, which is `tests/cli.luau`'s.
-- Every case reads and writes host files, in folders below the harness's `scratchRoot`.
local archive = require '@dream-archive'
local bytes = require '@dream/bytes'
local commands = require '@dream-archivetool/commands'
local fs = require '@dream/fs'
local test = require '@jess/test'
local tool = require '@dream-archivetool'
local expect = test.expect
local suite = test.new()

--- The lookup keys of the example archive's two entries, as every `pathBytesHex` spells them.
local IconHex = '69636f6e732f6578616d706c652e646473'
local MeshHex = '6d65736865732f6578616d706c652e6e6966'

--- What a call raises, as text.
local function failure(body: () -> ()): string
  local ok, problem = pcall(body)
  assert(not ok, 'expected the call to fail')
  return tostring(problem)
end

local counter = 0

--- A new folder for one case.
local function folder(): string
  local root = assert(rawget(_G, 'scratchRoot') :: string?, 'the case needs a scratchRoot')
  counter += 1
  local path = `{root}/commands-{counter}`
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

--- The TES3 archive the original's tests extract from: an icon and a mesh.
local function example(base: string): string
  local path = `{base}/test.bsa`
  local builder = archive.bsa.tes3.Builder.new()
  builder:addBytes('icons/example.dds', 'payload')
  builder:addBytes('meshes/example.nif', 'mesh')
  builder:writePath(path)
  return path
end

suite.describe('the commands that read', function()
  suite.it('info gives the archive\'s facts', function()
    local path = example(folder())
    expect(commands.info { archive = path }).toEqual {
      command = 'info',
      info = {
        path = path,
        format = 'bsaTes3',
        fileCount = 2,
        namedEntryCount = 2,
        hasUnnameableEntries = false,
        rewritable = true,
      },
    }
  end)

  suite.it('list gives every named entry, with its lookup key and size', function()
    local path = example(folder())
    expect(commands.list { archive = path }).toEqual {
      command = 'list',
      archive = path,
      entries = {
        { path = 'meshes/example.nif', pathBytesHex = MeshHex, size = 4 },
        { path = 'icons/example.dds', pathBytesHex = IconHex, size = 7 },
      },
    }
  end)

  suite.it('verify reports the index, and the payloads it read when asked to', function()
    local path = example(folder())
    local unread = commands.verify { archive = path }
    expect(unread.command).toBe 'verify'
    expect(unread.report.payloadsRead).toBeNil()
    expect(commands.verify({ archive = path, readPayloads = true }).report).toEqual {
      path = path,
      format = 'bsaTes3',
      fileCount = 2,
      namedEntryCount = 2,
      unnameableEntries = 0,
      rewritable = true,
      duplicateNormalizedPaths = {},
      unsafePaths = {},
      payloadsRead = 2,
      warnings = {},
    }
  end)

  suite.it('diff reports added, removed, and changed entries, by fingerprint when asked', function()
    local base = folder()
    write(`{base}/old/same.txt`, 'same')
    write(`{base}/old/changed.txt`, 'old')
    write(`{base}/old/gone.txt`, 'gone')
    write(`{base}/new/same.txt`, 'same')
    write(`{base}/new/changed.txt`, 'new')
    write(`{base}/new/added.txt`, 'added')
    local old, new = `{base}/old.bsa`, `{base}/new.bsa`
    commands.create { archive = old, input = `{base}/old`, format = 'tes3' }
    commands.create { archive = new, input = `{base}/new`, format = 'tes3' }

    -- The two payloads are the same size, so sizes alone can't tell them apart.
    local sized = commands.diff({ old = old, new = new }).report
    expect(sized.comparison).toBe 'metadataOnly'
    expect({ #sized.added, #sized.removed, #sized.changed, sized.unchanged }).toEqual { 1, 1, 0, 2 }

    local result = commands.diff { old = old, new = new, fingerprintPayloads = true }
    expect(result.command).toBe 'diff'
    expect(result.report).toEqual {
      old = old,
      new = new,
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
          path = 'gone.txt',
          pathBytesHex = '676f6e652e747874',
          size = 4,
          payloadFingerprint = bytes.fnv1a64 'gone',
        },
      },
      changed = {
        {
          path = 'changed.txt',
          pathBytesHex = '6368616e6765642e747874',
          old = { size = 3, payloadFingerprint = bytes.fnv1a64 'old' },
          new = { size = 3, payloadFingerprint = bytes.fnv1a64 'new' },
        },
      },
      unchanged = 1,
    }
  end)

  suite.it('raise for an archive that can\'t be opened, naming it', function()
    local missing = `{folder()}/missing.bsa`
    local unopened =
      `archive error: failed to open archive '{missing}': No such file or directory (os error 2)`
    expect(failure(function() commands.info { archive = missing } end)).toBe(unopened)
    expect(failure(function() commands.list { archive = missing } end)).toBe(unopened)
    expect(failure(function() commands.verify { archive = missing } end)).toBe(unopened)
    expect(failure(function() commands.diff { old = missing, new = missing } end)).toBe(unopened)
  end)
end)

suite.describe('extract', function()
  suite.it('gives an entry\'s bytes back for toMemory, and writes nothing', function()
    local base = folder()
    local path = example(base)
    expect(commands.extract { archive = path, entry = 'Icons\\Example.DDS', toMemory = true }).toEqual {
      command = 'extract',
      dryRun = false,
      conflicts = {},
      written = {},
      data = 'payload',
    }

    local hexed = commands.extract {
      archive = path,
      entryHex = IconHex,
      toMemory = true,
    }
    expect(hexed.data).toBe 'payload'
    expect(fs.list(base)).toEqual { 'test.bsa' }
  end)

  suite.it('reads an entry whose name isn\'t UTF-8 by its bytes', function()
    local path = `{folder()}/test.bsa`
    local builder = archive.bsa.tes3.Builder.new()
    builder:addBytes('textures/invalid-\255.dds', 'bytes')
    builder:writePath(path)
    local result =
      commands.extract { archive = path, entry = 'textures/invalid-\255.dds', toMemory = true }
    expect(result.data).toBe 'bytes'
    expect(commands.list({ archive = path }).entries[1].pathBytesHex).toBe '74657874757265732f696e76616c69642dff2e646473'
  end)

  suite.it('writes an entry under the output folder, and says what it decided and wrote', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    local row: tool.ExtractPlanRow = {
      action = 'extract',
      path = 'icons/example.dds',
      pathBytesHex = IconHex,
      target = `{out}/icons/example.dds`,
    }

    expect(commands.extract { archive = path, entry = 'icons/example.dds', output = out }).toEqual {
      command = 'extract',
      dryRun = false,
      plan = { operation = 'extract', archive = path, output = out, entries = { row } },
      conflicts = {},
      summary = { extracted = 1, skipped = 0 },
      written = { `{out}/icons/example.dds` },
    }
    expect(read(`{out}/icons/example.dds`)).toBe 'payload'
  end)

  suite.it('writes only the file name for flat', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    local result =
      commands.extract { archive = path, entry = 'icons/example.dds', output = out, flat = true }
    expect(result.written).toEqual { `{out}/example.dds` }
    expect(read(`{out}/example.dds`)).toBe 'payload'
    expect((fs.exists(`{out}/icons/example.dds`))).toBe(false)
  end)

  suite.it('stops at its plan for a dry run', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    local result = commands.extract {
      archive = path,
      entryHex = IconHex,
      output = out,
      dryRun = true,
    }
    expect(result.dryRun).toBe(true)
    expect(assert(result.plan).entries[1].target).toBe(`{out}/icons/example.dds`)
    expect(result.summary).toBeNil()
    expect(result.written).toEqual {}
    expect((fs.exists(out))).toBe(false)
  end)

  suite.it('reports a target that is already there, and follows the overwrite policy', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    write(`{out}/icons/example.dds`, 'existing')
    local request: commands.ExtractRequest =
      { archive = path, entry = 'icons/example.dds', output = out }

    local refused = commands.extract(request)
    expect(refused.summary).toBeNil()
    expect(refused.written).toEqual {}
    expect(refused.conflicts).toEqual {
      {
        action = 'conflict',
        path = 'icons/example.dds',
        pathBytesHex = IconHex,
        target = `{out}/icons/example.dds`,
      },
    }
    expect(read(`{out}/icons/example.dds`)).toBe 'existing'

    request.overwrite = 'skip'
    local skipped = commands.extract(request)
    expect(skipped.summary).toEqual { extracted = 0, skipped = 1 }
    expect(skipped.written).toEqual {}
    expect(assert(skipped.plan).entries[1].action).toBe 'skip'
    expect(read(`{out}/icons/example.dds`)).toBe 'existing'

    request.overwrite = 'overwrite'
    local replaced = commands.extract(request)
    expect(replaced.summary).toEqual { extracted = 1, skipped = 0 }
    expect(replaced.written).toEqual { `{out}/icons/example.dds` }
    expect(assert(replaced.plan).entries[1].action).toBe 'overwrite'
    expect(replaced.conflicts).toEqual {}
    expect(read(`{out}/icons/example.dds`)).toBe 'payload'
  end)

  suite.it('raises for an entry it can\'t extract, before making any folder', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    expect(
      failure(function() commands.extract { archive = path, entry = 'Missing.DDS', output = out } end)
    ).toBe 'archive entry not found: missing.dds'
    expect(
      failure(function() commands.extract { archive = path, entry = '../evil', output = out } end)
    ).toBe 'unsafe archive path: ../evil'
    expect(
      failure(function() commands.extract { archive = path, entryHex = 'abc', output = out } end)
    ).toBe 'planExtractByPathHex.entries[1]: invalid pathBytesHex: archive error: archive path hex must contain an even number of hexadecimal digits'
    expect(
      failure(function() commands.extract { archive = path, entryHex = 'zz', toMemory = true } end)
    ).toBe 'readEntryByPathHex: invalid pathBytesHex: archive error: archive path hex contains a non-hexadecimal digit'
    expect(
      failure(function() commands.extract { archive = path, entry = 'missing.dds', toMemory = true } end)
    ).toBe 'archive entry not found: missing.dds'
    expect((fs.exists(out))).toBe(false)
  end)

  suite.it('raises alike for a run and for a dry run', function()
    local path = example(folder())
    -- A folder's path names no entry, whether or not its last name could be written flat.
    local request: commands.ExtractRequest = { archive = path, entry = 'icons/', flat = true }
    expect(failure(function() commands.extract(request) end)).toBe 'archive entry not found: icons/'
    request.dryRun = true
    expect(failure(function() commands.extract(request) end)).toBe 'archive entry not found: icons/'
  end)

  suite.it('refuses a request it can\'t make sense of', function()
    local path = example(folder())
    expect(failure(function() commands.extract { archive = path } end)).toBe 'extract: no entry is named, as entry or as entryHex'
    expect(
      failure(function() commands.extract { archive = path, entry = 'a', entryHex = '61' } end)
    ).toBe 'extract: the entry is named twice, as entry and as entryHex'
    for _, request in
      {
        { archive = path, entry = 'icons/example.dds', toMemory = true, output = 'out' },
        { archive = path, entry = 'icons/example.dds', toMemory = true, flat = true },
        { archive = path, entry = 'icons/example.dds', toMemory = true, overwrite = 'skip' },
      } :: { commands.ExtractRequest }

    do
      expect(failure(function() commands.extract(request) end)).toBe 'extract: toMemory writes nothing, so it takes no output, flat, or overwrite'
    end
  end)
end)

suite.describe('extractAll', function()
  suite.it('plans every entry\'s target for a dry run, and writes none', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    expect(commands.extractAll { archive = path, output = out, dryRun = true }).toEqual {
      command = 'extractAll',
      dryRun = true,
      plan = {
        operation = 'extractAll',
        archive = path,
        output = out,
        entries = {
          {
            action = 'extract',
            path = 'meshes/example.nif',
            pathBytesHex = MeshHex,
            target = `{out}/meshes/example.nif`,
          },
          {
            action = 'extract',
            path = 'icons/example.dds',
            pathBytesHex = IconHex,
            target = `{out}/icons/example.dds`,
          },
        },
      },
      conflicts = {},
      written = {},
    }
    expect((fs.exists(out))).toBe(false)
  end)

  suite.it('extracts every entry, and lists the targets it wrote', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    local result = commands.extractAll { archive = path, output = out }
    expect(result.dryRun).toBe(false)
    expect(result.summary).toEqual { extracted = 2, skipped = 0 }
    expect(result.written).toEqual { `{out}/meshes/example.nif`, `{out}/icons/example.dds` }
    expect(read(`{out}/icons/example.dds`)).toBe 'payload'
    expect(read(`{out}/meshes/example.nif`)).toBe 'mesh'
  end)

  suite.it('writes nothing when a target is there, and skips it when told to', function()
    local base = folder()
    local path, out = example(base), `{base}/out`
    write(`{out}/meshes/example.nif`, 'existing')

    local planned = commands.extractAll { archive = path, output = out, dryRun = true }
    expect(planned.plan.entries[1].action).toBe 'conflict'
    expect(planned.plan.entries[2].action).toBe 'extract'
    expect(#planned.conflicts).toBe(1)

    local refused = commands.extractAll { archive = path, output = out }
    expect(refused.dryRun).toBe(false)
    expect(refused.summary).toBeNil()
    expect(refused.written).toEqual {}
    expect(refused.conflicts).toEqual { planned.plan.entries[1] }
    expect((fs.exists(`{out}/icons/example.dds`))).toBe(false)

    local skipped = commands.extractAll { archive = path, output = out, overwrite = 'skip' }
    expect(skipped.summary).toEqual { extracted = 1, skipped = 1 }
    expect(skipped.written).toEqual { `{out}/icons/example.dds` }
    expect(read(`{out}/meshes/example.nif`)).toBe 'existing'
    expect(read(`{out}/icons/example.dds`)).toBe 'payload'
  end)

  suite.it('raises for an archive whose entries have no names', function()
    local path = `{folder()}/hash.bsa`
    local builder = archive.bsa.tes4.Builder.new()
    builder:setNameMode 'hashOnly'
    builder:addBytes('meshes/a.nif', 'a')
    builder:writePath(path)
    expect(failure(function() commands.extractAll { archive = path, dryRun = true } end)).toBe 'archive error: archive contains entries without recoverable paths; refusing to extract it lossy'
  end)
end)

suite.describe('create', function()
  suite.it('plans the archive for a dry run, and writes none', function()
    local base = folder()
    write(`{base}/input/hello.txt`, 'hello')
    local output = `{base}/out.bsa`
    local request: commands.CreateRequest =
      { archive = output, input = `{base}/input`, format = 'tes3', dryRun = true }
    expect(commands.create(request)).toEqual {
      command = 'create',
      dryRun = true,
      plan = {
        operation = 'create',
        format = 'bsaTes3',
        output = output,
        files = 1,
        entries = {
          {
            action = 'add',
            source = `{base}/input/hello.txt`,
            path = 'hello.txt',
            pathBytesHex = '68656c6c6f2e747874',
            size = 5,
          },
        },
      },
      written = {},
    }
    expect((fs.exists(output))).toBe(false)
  end)

  suite.it('writes the archive, and says how many files it holds', function()
    local base = folder()
    write(`{base}/input/hello.txt`, 'hello')
    local output = `{base}/out.bsa`
    local result = commands.create { archive = output, input = `{base}/input`, format = 'tes3' }
    expect(result.dryRun).toBe(false)
    expect(result.files).toBe(1)
    expect(result.written).toEqual { output }
    expect(result.plan.files).toBe(1)
    expect(archive.openPath(output):readFile 'hello.txt').toBe 'hello'
  end)

  suite.it('writes each format with its own options', function()
    local base = folder()
    write(`{base}/input/hello.txt`, 'hello hello hello hello')
    local input = `{base}/input`
    local compressed = `{base}/compressed.bsa`
    commands.create { archive = compressed, input = input, format = 'tes4', compress = true }
    local facts = assert(commands.info({ archive = compressed }).info.tes4)
    expect(facts.version).toBe 'v103'
    expect(table.find(facts.archiveFlags, 'compressed') ~= nil).toBe(true)
    expect(archive.openPath(compressed):readFile 'hello.txt').toBe 'hello hello hello hello'

    local skyrim = `{base}/skyrim.bsa`
    commands.create { archive = skyrim, input = input, format = 'tes4', tes4Version = 'skyrimSe' }
    expect(assert(commands.info({ archive = skyrim }).info.tes4).version).toBe 'v105'

    local starfield = `{base}/starfield.ba2`
    local request: commands.CreateRequest =
      { archive = starfield, input = input, format = 'ba2', ba2Version = 'starfield' }
    expect(commands.create(request).plan.format).toBe 'ba2'
    expect(assert(commands.info({ archive = starfield }).info.ba2)).toEqual {
      version = 'v2',
      payloadFormat = 'gnrl',
      compressionFormat = 'zip',
      strings = true,
    }
  end)

  suite.it('raises for an option the format doesn\'t take, and an input it can\'t hold', function()
    local base = folder()
    write(`{base}/input/readme.txt`, 'payload')
    local input, output = `{base}/input`, `{base}/out.archive`
    local function refused(request: commands.CreateRequest): string
      return failure(function() commands.create(request) end)
    end

    expect(refused { archive = output, input = input, format = 'tes3', compress = true }).toBe 'compress is not valid with format bsaTes3'
    expect(refused { archive = output, input = input, format = 'tes3', ba2Kind = 'dx10' }).toBe 'ba2Kind is not valid with format bsaTes3'
    expect(refused { archive = output, input = input, format = 'tes4', ba2Version = 'starfield' }).toBe 'ba2Version is not valid with format bsaTes4'
    expect(refused { archive = output, input = input, format = 'ba2', tes4Version = 'skyrim' }).toBe 'tes4Version is not valid with format ba2'
    expect(refused { archive = output, input = input, format = 'ba2', ba2Kind = 'dx10' }).toBe 'archive error: DX10 BA2 archives can only contain .dds files: readme.txt'
    expect(refused { archive = output, input = input, format = 'ba2', ba2Kind = 'gnmf' }).toBe 'archive error: creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive'
    expect(refused { archive = output, input = `{base}/nowhere`, format = 'tes3' }).toBe 'I/O error: No such file or directory (os error 2)'
    expect(refused { archive = `{base}/nowhere/out.bsa`, input = input, format = 'tes3' }).toBe 'I/O error: No such file or directory (os error 2)'
    expect((fs.exists(output))).toBe(false)
  end)

  suite.it('refuses a symbolic link among the inputs unless told to follow it', function()
    local base = folder()
    write(`{base}/input/readme.txt`, 'payload')
    assert(fs.symlink('readme.txt', `{base}/input/link.txt`))
    local request: commands.CreateRequest =
      { archive = `{base}/out.bsa`, input = `{base}/input`, format = 'tes3' }
    expect(failure(function() commands.create(request) end)).toBe(
      `refusing to follow symlink input path: {base}/input/link.txt; set followSymlinks to opt in`
    )
    expect((fs.exists(`{base}/out.bsa`))).toBe(false)

    request.followSymlinks = true
    expect(commands.create(request).files).toBe(2)
    expect(archive.openPath(`{base}/out.bsa`):readFile 'link.txt').toBe 'payload'
  end)
end)

suite.describe('add', function()
  --- An archive of two files, and a folder that replaces one of them and adds a third.
  local function update(base: string): (string, string)
    write(`{base}/input/base.txt`, 'base')
    write(`{base}/input/keep.txt`, 'keep')
    write(`{base}/update/base.txt`, 'new')
    write(`{base}/update/added.txt`, 'added')
    local path = `{base}/base.bsa`
    commands.create { archive = path, input = `{base}/input`, format = 'tes3' }
    return path, `{base}/update`
  end

  suite.it('plans what it keeps, replaces, and adds for a dry run, and writes nothing', function()
    local base = folder()
    local path, inputs = update(base)
    local output = `{base}/updated.bsa`
    local before = read(path)
    expect(commands.add { archive = path, inputs = { inputs }, output = output, dryRun = true }).toEqual {
      command = 'add',
      dryRun = true,
      plan = {
        operation = 'add',
        archive = path,
        output = output,
        format = 'bsaTes3',
        files = 3,
        added = 1,
        replaced = 1,
        preserved = 1,
        entries = {
          { action = 'preserve', path = 'keep.txt', pathBytesHex = '6b6565702e747874' },
          {
            action = 'add',
            source = `{inputs}/added.txt`,
            path = 'added.txt',
            pathBytesHex = '61646465642e747874',
            size = 5,
          },
          {
            action = 'replace',
            source = `{inputs}/base.txt`,
            path = 'base.txt',
            pathBytesHex = '626173652e747874',
            size = 3,
          },
        },
      },
      written = {},
    }
    expect((fs.exists(output))).toBe(false)
    expect(read(path)).toBe(before)
  end)

  suite.it('writes to the output it is given, and leaves the archive as it was', function()
    local base = folder()
    local path, inputs = update(base)
    local output = `{base}/updated.bsa`
    local before = read(path)
    local result = commands.add { archive = path, inputs = { inputs }, output = output }
    expect(result.dryRun).toBe(false)
    expect(result.files).toBe(3)
    expect(result.written).toEqual { output }
    expect(read(path)).toBe(before)
    local updated = archive.openPath(output)
    expect(updated:readFile 'base.txt').toBe 'new'
    expect(updated:readFile 'keep.txt').toBe 'keep'
    expect(updated:readFile 'added.txt').toBe 'added'
  end)

  suite.it('replaces the archive without an output, taking files and folders alike', function()
    local base = folder()
    local path, inputs = update(base)
    write(`{base}/loose.txt`, 'loose')
    local result = commands.add { archive = path, inputs = { inputs, `{base}/loose.txt` } }
    expect(result.files).toBe(4)
    expect(result.plan.output).toBe(path)
    expect(result.written).toEqual { path }
    local updated = archive.openPath(path)
    expect(updated:readFile 'base.txt').toBe 'new'
    expect(updated:readFile 'loose.txt').toBe 'loose'
  end)

  suite.it('raises for what it can\'t rewrite or can\'t add', function()
    local base = folder()
    local path, inputs = update(base)
    local function refused(request: commands.AddRequest): string
      return failure(function() commands.add(request) end)
    end

    expect(refused { archive = path, inputs = { inputs }, output = path }).toBe 'archive error: explicit output archive path must differ from input archive path; omit output to replace the input archive'
    expect(refused { archive = path, inputs = { inputs, `{inputs}/added.txt` } }).toBe(
      `archive error: duplicate archive path after normalization: added.txt (first source: {inputs}/added.txt, duplicate source: {inputs}/added.txt)`
    )
    expect(refused { archive = path, inputs = {} }).toBe 'planAdd requires at least one input path'
    expect(refused { archive = path, inputs = { `{base}/nowhere` } }).toBe 'I/O error: No such file or directory (os error 2)'

    local hashed = `{base}/hash.bsa`
    local builder = archive.bsa.tes4.Builder.new()
    builder:setNameMode 'hashOnly'
    builder:writePath(hashed)
    expect(refused { archive = hashed, inputs = { inputs } }).toBe 'archive error: TES4 hash-only archives do not have recoverable path names; refusing to rewrite them lossy'
  end)
end)

return suite
