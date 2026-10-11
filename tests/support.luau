--!strict
--!native
-- What the archive tool's suite needs of the archive suites' support: a TES3 BSA laid out by hand
-- and a DDS file for a texture, as `dream-archive`'s `tests/support.luau` has them.

export type Writer = {
  u8: (value: number) -> (),
  u16: (value: number) -> (),
  u32: (value: number) -> (),
  u64: (value: number) -> (),
  raw: (value: string) -> (),
  at: () -> number,
  set32: (position: number, value: number) -> (),
  done: () -> string,
}

local function writer(): Writer
  local parts: { string } = {}
  local length = 0
  local patches: { { number } } = {}
  local function raw(value: string)
    table.insert(parts, value)
    length += #value
  end

  return {
    u8 = function(value: number) raw(string.pack('<I1', value % 256)) end,
    u16 = function(value: number) raw(string.pack('<I2', value % 0x10000)) end,
    u32 = function(value: number) raw(string.pack('<I4', value % 0x100000000)) end,
    u64 = function(value: number)
      raw(string.pack('<I4I4', value % 0x100000000, value // 0x100000000))
    end,
    raw = raw,
    at = function(): number return length end,
    set32 = function(position: number, value: number) table.insert(patches, { position, value }) end,
    done = function(): string
      local out = buffer.fromstring(table.concat(parts))
      for _, patch in patches do
        buffer.writeu32(out, patch[1], patch[2])
      end

      return buffer.tostring(out)
    end,
  }
end

export type Tes3File = { name: string, data: string }

--- A TES3 BSA of `files` in the given order, hashed by `hashFile`.
local function tes3(files: { Tes3File }, hashFile: (string) -> { lo: number, hi: number }): string
  local w = writer()
  local namesLength = 0
  for _, file in files do
    namesLength += #file.name + 1
  end

  w.u32(0x100)
  w.u32(12 * #files + namesLength)
  w.u32(#files)
  local offset = 0
  for _, file in files do
    w.u32(#file.data)
    w.u32(offset)
    offset += #file.data
  end

  local nameOffset = 0
  for _, file in files do
    w.u32(nameOffset)
    nameOffset += #file.name + 1
  end

  for _, file in files do
    w.raw(file.name .. '\0')
  end

  for _, file in files do
    local hash = hashFile(file.name)
    w.u32(hash.lo)
    w.u32(hash.hi)
  end

  for _, file in files do
    w.raw(file.data)
  end

  return w.done()
end

export type Texture = {
  height: number,
  width: number,
  mipCount: number,
  format: number,
  flags: number,
  tileMode: number,
}

local LegacyCodes: { [number]: string } = {
  [71] = 'DXT1',
  [74] = 'DXT3',
  [77] = 'DXT5',
  [80] = 'BC4U',
  [81] = 'BC4S',
  [83] = 'BC5U',
  [84] = 'BC5S',
}

--- The plain pixel formats: flags, bit count, and the four masks.
local PlainFormats: { [number]: { number } } = {
  [28] = { 0x41, 32, 0xff, 0xff00, 0xff0000, 0xff000000 },
  [87] = { 0x41, 32, 0xff0000, 0xff00, 0xff, 0xff000000 },
  [88] = { 0x40, 32, 0xff0000, 0xff00, 0xff, 0 },
  [85] = { 0x40, 16, 0xf800, 0x7e0, 0x1f, 0 },
  [86] = { 0x41, 16, 0x7c00, 0x3e0, 0x1f, 0x8000 },
  [49] = { 0x20001, 16, 0xff, 0, 0, 0xff00 },
  [65] = { 0x2, 8, 0, 0, 0, 0xff },
  [61] = { 0x20000, 8, 0xff, 0, 0, 0 },
}

--- A DDS file for `texture` with `payload`: a legacy FourCC header, a plain pixel format, or
--- the DX10 extension (`style` picks; `arraySize` and `volume` make the ones the builder refuses).
local function ddsFile(
  texture: Texture,
  payload: string,
  style: string,
  arraySize: number?,
  volume: boolean?
): string
  local header = buffer.create(148)
  buffer.writestring(header, 0, 'DDS ')
  buffer.writeu32(header, 4, 124)
  buffer.writeu32(header, 8, 0x1007 + (if volume then 0x800000 else 0))
  buffer.writeu32(header, 12, texture.height)
  buffer.writeu32(header, 16, texture.width)
  buffer.writeu32(header, 24, 1)
  buffer.writeu32(header, 28, texture.mipCount)
  buffer.writeu32(header, 76, 32)
  local cube = bit32.band(texture.flags, 1) ~= 0
  buffer.writeu32(header, 112, if cube then 0xfe00 else 0)
  local length = 128
  if style == 'legacy' and LegacyCodes[texture.format] then
    buffer.writeu32(header, 80, 4)
    buffer.writestring(header, 84, LegacyCodes[texture.format])
  elseif style == 'plain' and PlainFormats[texture.format] then
    local plain = PlainFormats[texture.format]
    buffer.writeu32(header, 80, plain[1])
    for index = 2, 6 do
      buffer.writeu32(header, 84 + 4 * (index - 1), plain[index])
    end
  else
    buffer.writeu32(header, 80, 4)
    buffer.writestring(header, 84, 'DX10')
    buffer.writeu32(header, 128, texture.format)
    buffer.writeu32(header, 132, 3)
    buffer.writeu32(header, 136, if cube then 4 else 0)
    buffer.writeu32(header, 140, arraySize or 1)
    length = 148
  end

  return buffer.readstring(header, 0, length) .. payload
end

return table.freeze {
  tes3 = tes3,
  ddsFile = ddsFile,
}
