--!strict
--!native
-- SPDX-License-Identifier: MIT OR Apache-2.0
-- JSON as dream_archivetool writes it, which is serde_json's pretty form: an object's fields in
-- the order they were given, one field or item to a line, two spaces a level, and an empty array
-- or object closed on its own line. Text is escaped as serde_json escapes it: the quote, the
-- backslash, and the control characters, with everything else left as the bytes it is.
local bytes = require '@dream/bytes'

--- An object's fields or an array's items, in order, or the null that stands for no value.
export type Node = {
  kind: 'object' | 'array' | 'null',
  fields: { Field }?,
  items: { Value }?,
}

export type Value = string | number | boolean | Node

export type Field = { name: string, value: Value }

local Null: Node = { kind = 'null' }

local Escapes: { [string]: string } = {
  ['"'] = '\\"',
  ['\\'] = '\\\\',
  ['\b'] = '\\b',
  ['\f'] = '\\f',
  ['\n'] = '\\n',
  ['\r'] = '\\r',
  ['\t'] = '\\t',
}

local function escape(character: string): string
  return Escapes[character] or string.format('\\u%04x', string.byte(character))
end

--- What `quoted` escapes: the control characters, the quote, and the backslash, as bytes.
local Escaped = bytes.regex('[\\x00-\\x1f"\\\\]', { unicode = false })

local function quoted(text: string): string
  if not Escaped:isMatch(text) then return `"{text}"` end

  return `"{string.gsub(text, '[%z\1-\31"\\]', escape)}"`
end

--- An object of `fields`, each a name and its value; a field whose value is nil is left out, as
--- serde leaves out one it was told to skip.
local function object(fields: { { unknown } }): Node
  local kept: { Field } = {}
  for _, field in fields do
    local value = field[2]
    if value ~= nil then table.insert(kept, { name = field[1] :: string, value = value :: Value }) end
  end

  return { kind = 'object', fields = kept }
end

local function array(items: { Value }): Node return { kind = 'array', items = items } end

--- `value`, or the null that stands for its absence.
local function optional(value: Value?): Value
  if value == nil then return Null end

  return value
end

local function write(parts: { string }, value: Value, indent: string)
  if type(value) == 'string' then
    table.insert(parts, quoted(value))
    return
  end

  if type(value) == 'number' then
    table.insert(parts, string.format('%d', value))
    return
  end

  if type(value) == 'boolean' then
    table.insert(parts, if value then 'true' else 'false')
    return
  end

  if value.kind == 'null' then
    table.insert(parts, 'null')
    return
  end

  local inner = `{indent}  `
  if value.kind == 'array' then
    local items = value.items :: { Value }
    if #items == 0 then
      table.insert(parts, '[]')
      return
    end

    table.insert(parts, '[\n')
    for index, item in items do
      table.insert(parts, inner)
      write(parts, item, inner)
      table.insert(parts, if index < #items then ',\n' else '\n')
    end

    table.insert(parts, indent .. ']')
    return
  end

  local fields = value.fields :: { Field }
  if #fields == 0 then
    table.insert(parts, '{}')
    return
  end

  table.insert(parts, '{\n')
  for index, field in fields do
    table.insert(parts, `{inner}{quoted(field.name)}: `)
    write(parts, field.value, inner)
    table.insert(parts, if index < #fields then ',\n' else '\n')
  end

  table.insert(parts, indent .. '}')
end

--- `value` as serde_json's `to_writer_pretty` writes it, without a newline after it.
local function pretty(value: Value): string
  local parts: { string } = {}
  write(parts, value, '')
  return table.concat(parts)
end

return table.freeze {
  null = Null,
  object = object,
  array = array,
  optional = optional,
  pretty = pretty,
}
