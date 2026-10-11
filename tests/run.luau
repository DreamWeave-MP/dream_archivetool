--!strict
--!native
-- The archive tool's own suite: dream_archivetool's tests, held to the port alone. The cases on
-- archives opened from bytes always run; the ones that read and write host files run when the
-- harness names a scratch folder as `scratchRoot`.
local archive = require '@dream-archive'
local bytes = require '@dream/bytes'
local fs = require '@dream/fs'
local support = require './support'
local test = require '@jess/test'
local tool = require '@dream-archivetool'
local expect = test.expect
local suite = test.new()

local anyTool: any = tool

--- What a call raises, as text.
local function failure(body: () -> ()): string
  local ok, problem = pcall(body)
  assert(not ok, 'expected the call to fail')
  return tostring(problem)
end

local function tes3(files: { { string } }): archive.Archive
  local builder = archive.bsa.tes3.Builder.new()
  for _, file in files do
    builder:addBytes(file[1], file[2])
  end

  return archive.openBytes(builder:toBytes())
end

suite.describe('the module', function()
  suite.it('has the native module functions, frozen', function()
    for _, name in
      {
        'info',
        'verify',
        'diff',
        'extract',
        'extractByPathHex',
        'extractHex',
        'extractMany',
        'extractManyByPathHex',
        'planExtract',
        'planExtractByPathHex',
        'extractAll',
        'planExtractAll',
        'create',
        'planCreate',
        'add',
        'planAdd',
      }

    do
      expect(type(anyTool[name])).toBe 'function'
    end

    expect(tool.version).toBe '1.1.0'
    expect(require '@dream-archivetool').toBe(tool)
    expect(function() anyTool.info = nil end).toThrow()
    expect(anyTool.toolInfo).toBeNil()
  end)

  suite.it('checks its arguments as the binding did', function()
    expect(failure(function() anyTool.info() end)).toBe 'dream.archivetool.info: bad argument count (expected at least 1, got 0)'
    expect(failure(function() anyTool.info(5) end)).toBe 'dream.archivetool.info: bad argument #1 (expected string): Lua stack index 1: expected string, got number'
    expect(failure(function() anyTool.extract 'x.bsa' end)).toBe 'dream.archivetool.extract: bad argument count (expected at least 2, got 1)'
  end)
end)

suite.describe('archives in memory', function()
  suite.it('reports info, labelled <memory>', function()
    local info = tool.info(tes3 { { 'meshes/example.nif', 'payload' } })
    expect(info).toEqual {
      path = '<memory>',
      format = 'bsaTes3',
      fileCount = 1,
      namedEntryCount = 1,
      hasUnnameableEntries = false,
      rewritable = true,
    }
  end)

  suite.it('gives TES4 header facts in Luau spellings', function()
    local builder = archive.bsa.tes4.Builder.new()
    builder:setVersion(104)
    builder:setNameMode 'stringsAndEmbedded'
    builder:setCompressed(true)
    builder:addBytes('meshes/a.nif', 'a')
    local info = tool.info(archive.openBytes(builder:toBytes()))
    local facts = info.tes4 :: any
    expect(facts.version).toBe 'v104'
    expect(facts.nameMode).toBe 'stringsAndEmbedded'
    expect(facts.archiveFlags).toEqual {
      'directoryStrings',
      'fileStrings',
      'compressed',
      'embeddedFileNames',
    }
    expect(facts.archiveTypes).toBe(`ArchiveTypes({facts.archiveTypesBits})`)
    expect(facts.unsupportedArchiveFlagsBits).toBe(0)
  end)

  suite.it('refuses to rewrite a hash-only TES4 archive', function()
    local builder = archive.bsa.tes4.Builder.new()
    builder:setNameMode 'hashOnly'
    builder:addBytes('meshes/a.nif', 'a')
    local info = tool.info(archive.openBytes(builder:toBytes()))
    expect(info.rewritable).toBe(false)
    expect(info.hasUnnameableEntries).toBe(true)
    expect(info.rewriteBlocker).toBe 'archive contains entries without recoverable paths; refusing to rewrite it lossy'
  end)

  suite.it('reports unsafe paths by normalized key beside the stored bytes', function()
    local builder = archive.bsa.tes3.Builder.new()
    builder:addBytes('ab\\cd\\C.dds', 'payload')
    local data = builder:toBytes()
    local at = string.find(string.lower(data), 'ab\\cd\\c.dds', 1, true) :: number
    data = string.sub(data, 1, at - 1) .. '..\\..\\C.dds' .. string.sub(data, at + 11)
    local report = tool.verify(archive.openBytes(data))
    expect(report.unsafePaths).toEqual {
      {
        path = '../../c.dds',
        pathBytesHex = '2e2e2f2e2e2f632e646473',
        rawPathBytesHex = '2e2e5c2e2e5c432e646473',
      },
    }
    expect(report.warnings).toEqual { 'archive contains paths unsafe to extract directly' }
  end)

  suite.it('reports a duplicate after the entry it collides with', function()
    local data = support.tes3({
      { name = 'Textures/Example.dds', data = 'one' },
      { name = 'textures/example.dds', data = 'two' },
    }, archive.bsa.tes3.hashFile)
    local report = tool.verify(archive.openBytes(data), { readPayloads = true })
    local duplicate = report.duplicateNormalizedPaths[1]
    expect(duplicate.pathBytesHex).toBe '74657874757265732f6578616d706c652e646473'
    expect(report.payloadsRead).toBeNil()
    expect(#report.warnings).toBe(2)
  end)

  suite.it('lists TES4 sizes in every name mode, compressed or not', function()
    for _, case: any in
      {
        { 103, 'strings', false },
        { 104, 'stringsAndEmbedded', false },
        { 104, 'embedded', true },
        { 105, 'strings', true },
      }

    do
      local builder = archive.bsa.tes4.Builder.new()
      builder:setVersion(case[1])
      builder:setNameMode(case[2])
      builder:setCompressed(case[3])
      builder:addBytes('meshes/a.nif', string.rep('a', 300))
      builder:addBytes('meshes/b.nif', 'bb')
      local empty = archive.bsa.tes4.Builder.new()
      local report =
        tool.diff(archive.openBytes(empty:toBytes()), archive.openBytes(builder:toBytes()))
      expect(#report.added).toBe(2)
      for _, entry in report.added do
        expect(entry.size).toBe(if entry.path == 'meshes/a.nif' then 300 else 2)
        expect(entry.compressedSize ~= nil).toBe(case[3])
      end
    end
  end)

  suite.it('compares uncompressed TES4 archives by size, and payloads by FNV-1a', function()
    local function write(payload: string): archive.Archive
      local builder = archive.bsa.tes4.Builder.new()
      builder:addBytes('meshes/same.nif', 'same')
      builder:addBytes('meshes/grown.nif', payload)
      return archive.openBytes(builder:toBytes())
    end

    local old, new = write 'old', write 'newer'
    local report = tool.diff(old, new)
    expect(report.comparison).toBe 'metadataOnly'
    expect(report.unchanged).toBe(1)
    expect(report.changed[1].path).toBe 'meshes/grown.nif'
    expect(report.changed[1].old.size).toBe(3)
    expect(report.changed[1].new.size).toBe(5)

    local same = tool.diff(write 'abc', write 'abd', { fingerprintPayloads = true })
    expect(same.comparison).toBe 'payloadFingerprint'
    expect(same.changed[1].old.payloadFingerprint).toBe(bytes.fnv1a64 'abc')
    expect(same.changed[1].new.payloadFingerprint).toBe(bytes.fnv1a64 'abd')
  end)

  suite.it('plans refuse a missing member and report an existing target', function()
    local opened = tes3 { { 'textures/a.dds', 'a' }, { 'meshes/b.nif', 'b' } }
    expect(failure(function() tool.planExtract(opened, { 'textures/a.dds', 'missing.dds' }) end)).toBe 'archive entry not found: missing.dds'
    local plan = tool.planExtract(opened, { 'TEXTURES//A.DDS' }, { output = 'out' })
    expect(plan).toEqual {
      operation = 'extract',
      archive = '<memory>',
      output = 'out',
      entries = {
        {
          action = 'extract',
          path = 'textures/a.dds',
          pathBytesHex = '74657874757265732f612e646473',
          target = 'out/textures/a.dds',
        },
      },
    }
    local flat = tool.planExtractByPathHex(
      opened,
      { '74657874757265732f612e646473' },
      { preservePaths = false }
    )
    expect(flat.entries[1].target).toBe './a.dds'
  end)

  suite.it('refuses unsafe paths and hash-only entries as targets', function()
    local opened = tes3 { { 'textures/a.dds', 'a' } }
    for _, path in
      {
        '',
        '/absolute.txt',
        'textures/../evil.txt',
        'textures/has\0nul.txt',
        'C:/evil.txt',
        '.',
        './.',
      }

    do
      expect(failure(function() tool.planExtract(opened, { path }) end)).toContain 'unsafe archive path: '
    end

    local builder = archive.bsa.tes4.Builder.new()
    builder:setNameMode 'hashOnly'
    builder:addBytes('meshes/a.nif', 'a')
    local hashed = archive.openBytes(builder:toBytes())
    expect(failure(function() tool.planExtract(hashed, hashed:entries()) end)).toBe 'planExtract.entries[1]: entry 1 has no path; hash-only entries are not extraction targets'
    expect(failure(function() tool.planExtractAll(hashed) end)).toBe 'archive error: archive contains entries without recoverable paths; refusing to extract it lossy'
  end)

  suite.it('lists named entries in archive order, with the sizes the format records', function()
    local opened = tes3 { { 'Textures\\Example.DDS', 'payload' }, { 'meshes/a.nif', 'mesh' } }
    expect(tool.list(opened)).toEqual {
      {
        path = 'textures/example.dds',
        pathBytesHex = '74657874757265732f6578616d706c652e646473',
        size = 7,
      },
      { path = 'meshes/a.nif', pathBytesHex = '6d65736865732f612e6e6966', size = 4 },
    }

    local compressed = archive.bsa.tes4.Builder.new()
    compressed:setCompressed(true)
    compressed:addBytes('meshes/a.nif', string.rep('a', 300))
    expect(tool.list(archive.openBytes(compressed:toBytes()))).toEqual {
      {
        path = 'meshes/a.nif',
        pathBytesHex = '6d65736865732f612e6e6966',
        size = 300,
        compressedSize = 27,
      },
    }

    local general = archive.ba2.Builder.new()
    general:addBytes('Sound/Voice.fuz', 'voice')
    expect(tool.list(archive.openBytes(general:toBytes()))).toEqual {
      { path = 'sound/voice.fuz', pathBytesHex = '736f756e642f766f6963652e66757a', size = 5 },
    }

    local hashed = archive.bsa.tes4.Builder.new()
    hashed:setNameMode 'hashOnly'
    hashed:addBytes('meshes/a.nif', 'a')
    expect(tool.list(archive.openBytes(hashed:toBytes()))).toEqual {}
    expect(failure(function() anyTool.list() end)).toBe 'dream.archivetool.list: bad argument count (expected at least 1, got 0)'
  end)

  suite.it('reads an entry by any spelling of its path, by its entry, and by its hex', function()
    local opened = tes3 { { 'Textures\\Example.DDS', 'payload' }, { 'meshes/a.nif', 'mesh' } }
    expect(tool.readEntry(opened, 'TEXTURES//example.dds')).toBe 'payload'
    expect(tool.readEntry(opened, opened:entries()[2])).toBe 'mesh'
    expect(tool.readEntryByPathHex(opened, '6d65736865732f612e6e6966')).toBe 'mesh'
    expect(failure(function() tool.readEntry(opened, 'Missing.DDS') end)).toBe 'archive entry not found: missing.dds'
    -- Nothing is written, so a path extraction would refuse is only one that isn't there.
    expect(failure(function() tool.readEntry(opened, '../evil') end)).toBe 'archive entry not found: ../evil'
    expect(failure(function() tool.readEntry(opened, '') end)).toBe 'unsafe archive path: '
    expect(failure(function() tool.readEntryByPathHex(opened, 'abc') end)).toBe 'readEntryByPathHex: invalid pathBytesHex: archive error: archive path hex must contain an even number of hexadecimal digits'
    expect(failure(function() tool.readEntryByPathHex(opened, 'zz') end)).toBe 'readEntryByPathHex: invalid pathBytesHex: archive error: archive path hex contains a non-hexadecimal digit'
    expect(failure(function() anyTool.readEntry(opened) end)).toBe 'dream.archivetool.readEntry: bad argument count (expected at least 2, got 1)'
    expect(failure(function() anyTool.readEntry(opened, 5) end)).toBe 'readEntry: expected an archive path or an entry handle, got number'
  end)

  suite.it('reads option tables strictly', function()
    local opened = tes3 { { 'textures/a.dds', 'a' } }
    expect(
      failure(
        function() anyTool.planExtract(opened, { 'textures/a.dds' }, { overwirte = 'skip' }) end
      )
    ).toBe 'planExtract: unknown option \'overwirte\'; known options are fsync, output, overwrite, preservePaths'
    expect(
      failure(function() anyTool.planExtract(opened, { 'textures/a.dds' }, { output = 5 }) end)
    ).toBe 'planExtract.output: expected string, got number'
    expect(
      failure(
        function() anyTool.planExtract(opened, { 'textures/a.dds' }, { overwrite = 'explode' }) end
      )
    ).toBe 'planExtract.overwrite: unknown overwrite mode: explode'
    expect(failure(function() anyTool.planExtractAll(opened, { preservePaths = true }) end)).toBe 'planExtractAll: unknown option \'preservePaths\'; known options are fsync, output, overwrite'
    expect(failure(function() anyTool.planExtract(opened, { [1] = 'a', [3] = 'b' }) end)).toBe 'planExtract.entries must be a dense 1-based array'
    expect(failure(function() anyTool.planExtractByPathHex(opened, { 'zz' }) end)).toBe 'planExtractByPathHex.entries[1]: invalid pathBytesHex: archive error: archive path hex contains a non-hexadecimal digit'
    expect(failure(function() anyTool.verify(opened, { fsync = true, readPayloads = true }) end)).toBe 'verify: unknown option \'fsync\'; known options are readPayloads'
    expect(failure(function() anyTool.planExtractAll(opened, { fsync = true }) end)).toBe 'planExtractAll.fsync: flushing to disk is not available in @dream-archivetool'
  end)
end)

--- The host filesystem cases, in folders below `root`.
local function filesystemSuite(root: string)
  local counter = 0
  local function folder(): string
    counter += 1
    local path = `{root}/tool-{counter}`
    assert(fs.mkdir(path, { recursive = true }))
    return path
  end

  local function writeTes3(path: string, files: { { string } })
    local builder = archive.bsa.tes3.Builder.new()
    for _, file in files do
      builder:addBytes(file[1], file[2])
    end

    builder:writePath(path)
  end

  local function dds(payload: string): string
    local header = { height = 4, width = 4, mipCount = 1, format = 71, flags = 0, tileMode = 0 }
    return support.ddsFile(header, payload, 'dx10')
  end

  suite.it('lists and reads an archive named by its path', function()
    local base = folder()
    local path, missing = `{base}/test.bsa`, `{base}/missing.bsa`
    writeTes3(path, { { 'textures/a.dds', 'a' }, { 'meshes/b.nif', 'b' } })
    -- Archive order is the order of the names' hashes, which a TES3 BSA is sorted by.
    expect(tool.list(path)).toEqual {
      { path = 'meshes/b.nif', pathBytesHex = '6d65736865732f622e6e6966', size = 1 },
      { path = 'textures/a.dds', pathBytesHex = '74657874757265732f612e646473', size = 1 },
    }
    expect(tool.readEntry(path, 'Meshes\\B.nif')).toBe 'b'
    expect(tool.readEntryByPathHex(path, '6d65736865732f622e6e6966')).toBe 'b'
    local unopened =
      `archive error: failed to open archive '{missing}': No such file or directory (os error 2)`
    expect(failure(function() tool.list(missing) end)).toBe(unopened)
    -- The archive is opened before the path is looked at, and the hex is read before either.
    expect(failure(function() tool.readEntry(missing, '') end)).toBe(unopened)
    expect(failure(function() tool.readEntryByPathHex(missing, 'abc') end)).toBe 'readEntryByPathHex: invalid pathBytesHex: archive error: archive path hex must contain an even number of hexadecimal digits'
  end)

  suite.it('extracts one entry to a safe path, by its stored spelling', function()
    local base = folder()
    local path = `{base}/test.bsa`
    writeTes3(path, { { 'textures/example.dds', 'payload' } })
    local summary = tool.extract(path, 'TEXTURES//EXAMPLE.DDS', { output = `{base}/out` })
    expect(summary).toEqual { extracted = 1, skipped = 0 }
    expect((fs.readFileString(`{base}/out/textures/example.dds`))).toBe 'payload'
    expect(
      failure(function() tool.extract(path, 'textures/example.dds', { output = `{base}/out` }) end)
    ).toBe(`target already exists: {base}/out/textures/example.dds`)
    assert(fs.writeFile(`{base}/out/textures/example.dds`, 'existing'))
    expect(
      tool.extract(path, 'textures/example.dds', { output = `{base}/out`, overwrite = 'skip' })
    ).toEqual { extracted = 0, skipped = 1 }
    expect((fs.readFileString(`{base}/out/textures/example.dds`))).toBe 'existing'
    expect(
      tool.extract(path, 'textures/example.dds', { output = `{base}/out`, overwrite = 'overwrite' })
    ).toEqual { extracted = 1, skipped = 0 }
    expect((fs.readFileString(`{base}/out/textures/example.dds`))).toBe 'payload'
    tool.extract(path, 'textures\\example.dds', { output = `{base}/flat`, preservePaths = false })
    expect((fs.readFileString(`{base}/flat/example.dds`))).toBe 'payload'
  end)

  suite.it('refuses a missing member before making any folder', function()
    local base = folder()
    local path = `{base}/test.bsa`
    writeTes3(path, { { 'textures/a.dds', 'a' }, { 'meshes/b.nif', 'b' } })
    local options: tool.ExtractOptions = { output = `{base}/out` }
    expect(
      failure(function() tool.extractMany(path, { 'textures/a.dds', 'missing.dds' }, options) end)
    ).toBe 'archive entry not found: missing.dds'
    expect(failure(function() tool.extract(path, 'Missing.DDS', options) end)).toBe 'archive entry not found: missing.dds'
    expect((fs.exists(`{base}/out`))).toBe(false)
  end)

  suite.it('extracts everything, skipping or refusing what is there before writing', function()
    local base = folder()
    local path = `{base}/test.bsa`
    writeTes3(path, { { 'textures/a.dds', 'a' }, { 'meshes/b.nif', 'b' } })
    local out = `{base}/out`
    assert(fs.mkdir(`{out}/meshes`, { recursive = true }))
    assert(fs.writeFile(`{out}/meshes/b.nif`, 'existing'))
    local plan = tool.planExtractAll(path, { output = out })
    for _, row in plan.entries do
      expect(row.action).toBe(if row.path == 'meshes/b.nif' then 'conflict' else 'extract')
    end

    expect(failure(function() tool.extractAll(path, { output = out }) end)).toBe(
      `target already exists: {out}/meshes/b.nif`
    )
    expect((fs.exists(`{out}/textures/a.dds`))).toBe(false)
    expect(tool.extractAll(path, { output = out, overwrite = 'skip' })).toEqual {
      extracted = 1,
      skipped = 1,
    }
    expect((fs.readFileString(`{out}/meshes/b.nif`))).toBe 'existing'
    expect((fs.readFileString(`{out}/textures/a.dds`))).toBe 'a'
  end)

  suite.it('creates every format, and refuses what a format can not hold', function()
    local base = folder()
    assert(fs.mkdir(`{base}/input/textures`, { recursive = true }))
    assert(fs.writeFile(`{base}/input/textures/example.dds`, 'payload'))
    for _, options in
      {
        { format = 'tes3' },
        { format = 'tes4' },
        { format = 'tes4', compress = true },
        { format = 'ba2' },
        { format = 'ba2', compress = true },
        { format = 'ba2', ba2Version = 'starfield' },
        { format = 'ba2', ba2Version = 'fallout4NextGen' },
      } :: { tool.CreateOptions }

    do
      local output = `{base}/out.archive`
      expect(tool.create(output, `{base}/input`, options)).toEqual { files = 1 }
      expect(archive.openPath(output):readFile 'textures/example.dds').toBe 'payload'
    end

    local existing, input = `{base}/existing.ba2`, `{base}/input`
    assert(fs.writeFile(existing, 'existing'))
    assert(fs.writeFile(`{input}/readme.txt`, 'readme'))
    local dx10 = failure(
      function() tool.create(existing, input, { format = 'ba2', ba2Kind = 'dx10' }) end
    )
    expect(dx10).toBe 'archive error: DX10 BA2 archives can only contain .dds files: readme.txt'
    local gnmf = failure(
      function() tool.create(existing, input, { format = 'ba2', ba2Kind = 'gnmf' }) end
    )
    expect(gnmf).toContain 'creating or updating GNMF BA2 archives requires console texture swizzle'
    expect((fs.readFileString(existing))).toBe 'existing'
    local compressed = failure(
      function() tool.create(`{base}/x.bsa`, input, { compress = true }) end
    )
    expect(compressed).toBe 'compress is not valid with format bsaTes3'

    assert(fs.mkdir(`{base}/upper`, { recursive = true }))
    assert(fs.writeFile(`{base}/upper/A.DDS`, dds 'aaaaaaaa'))
    expect(tool.create(`{base}/upper.ba2`, `{base}/upper`, { format = 'ba2', ba2Kind = 'dx10' })).toEqual {
      files = 1,
    }
  end)

  suite.it('adds to an archive, keeping, replacing, and adding', function()
    local base = folder()
    local path = `{base}/test.bsa`
    writeTes3(path, { { 'textures/a.dds', 'old a' }, { 'meshes/b.nif', 'b' } })
    assert(fs.mkdir(`{base}/more/textures`, { recursive = true }))
    assert(fs.writeFile(`{base}/more/textures/a.dds`, 'new a'))
    assert(fs.writeFile(`{base}/more/c.txt`, 'c'))
    local plan = tool.planAdd(path, { inputs = { `{base}/more` } })
    expect(plan.output).toBe(path)
    expect({ plan.files, plan.added, plan.replaced, plan.preserved }).toEqual { 3, 1, 1, 1 }
    expect(failure(function() tool.add(path, { inputs = { `{base}/more` }, output = path }) end)).toBe 'archive error: explicit output archive path must differ from input archive path; omit output to replace the input archive'
    expect(tool.add(path, { inputs = { `{base}/more` } })).toEqual { files = 3 }
    local updated = archive.openPath(path)
    expect(updated:readFile 'textures/a.dds').toBe 'new a'
    expect(updated:readFile 'meshes/b.nif').toBe 'b'
    expect(updated:readFile 'c.txt').toBe 'c'
    expect(
      failure(
        function() tool.planAdd(path, { inputs = { `{base}/more`, `{base}/more/c.txt` } }) end
      )
    ).toBe(
      `archive error: duplicate archive path after normalization: c.txt (first source: {base}/more/c.txt, duplicate source: {base}/more/c.txt)`
    )
  end)

  suite.it('keeps DX10 textures and their archive settings when adding', function()
    local base = folder()
    assert(fs.mkdir(`{base}/one`, { recursive = true }))
    assert(fs.mkdir(`{base}/two`, { recursive = true }))
    assert(fs.writeFile(`{base}/one/a.dds`, dds 'aaaaaaaa'))
    assert(fs.writeFile(`{base}/two/b.dds`, dds 'bbbbbbbb'))
    tool.create(
      `{base}/tex.ba2`,
      `{base}/one`,
      { format = 'ba2', ba2Kind = 'dx10', ba2Version = 'fallout4NextGen' }
    )
    expect(tool.add(`{base}/tex.ba2`, { inputs = { `{base}/two` } })).toEqual { files = 2 }
    local updated = archive.openPath(`{base}/tex.ba2`)
    expect(updated:info().version).toBe(8)
    -- A texture comes back with the header the archive rebuilds; its pixels are what went in.
    local kept, added = updated:readFileRequired 'a.dds', updated:readFileRequired 'b.dds'
    expect(string.sub(kept, -8)).toBe 'aaaaaaaa'
    expect(string.sub(added, -8)).toBe 'bbbbbbbb'
  end)

  suite.it('refuses an archive it can not rewrite', function()
    local base = folder()
    local builder = archive.bsa.tes4.Builder.new()
    builder:setNameMode 'hashOnly'
    builder:writePath(`{base}/hash.bsa`)
    assert(fs.writeFile(`{base}/x.txt`, 'x'))
    expect(failure(function() tool.add(`{base}/hash.bsa`, { inputs = { `{base}/x.txt` } }) end)).toBe 'archive error: TES4 hash-only archives do not have recoverable path names; refusing to rewrite them lossy'
  end)
end

local scratch = rawget(_G :: any, 'scratchRoot') :: string?
if scratch then
  suite.describe('on the host filesystem', function() filesystemSuite(scratch :: string) end)
end

return suite
