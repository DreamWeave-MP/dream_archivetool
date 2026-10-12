--!strict
--!native
-- SPDX-License-Identifier: MIT OR Apache-2.0
-- A command's result as the text dream_archivetool prints for it: the plain lines, or with
-- `json` the report as serde wrote the crate's structs, whose field names are snake_case and
-- whose enumerated values are kebab-case where `@dream-archivetool`'s tables are camelCase.
-- Data in, text out: nothing here reads a file or writes a stream.
local commands = require '@dream-archivetool/commands'
local json = require './json'
local tool = require '@dream-archivetool'
local util = require '@dream-archive/util'

export type Options = {
  --- The report as JSON, where the command has plain text too.
  json: boolean?,
  --- `list`'s sizes beside its paths.
  long: boolean?,
}

--- The archive families as the command line spells them.
local Formats: { [string]: string } = { bsaTes3 = 'tes3', bsaTes4 = 'tes4', ba2 = 'ba2' }

--- A camelCase name as serde's `kebab-case` renames it: `extractAll` is `extract-all`.
local function kebab(name: string): string
  return (
    string.gsub(
      name,
      '%u',
      function(letter: string): string return `-{string.lower(letter)}` end
    )
  )
end

local function document(value: json.Value): string return `{json.pretty(value)}\n` end

local function list<Row>(rows: { Row }, shape: (Row) -> json.Value): json.Value
  return json.array([for row in rows => shape(row)])
end

local function names(values: { string }): json.Value
  return list(values, function(value: string): json.Value return value end)
end

local function infoShape(info: tool.ArchiveInfo): json.Value
  local {tes4, ba2} = info
  return json.object {
    { 'path', info.path },
    { 'format', Formats[info.format] },
    { 'file_count', info.fileCount },
    { 'named_entry_count', info.namedEntryCount },
    { 'has_unnameable_entries', info.hasUnnameableEntries },
    { 'rewritable', info.rewritable },
    { 'rewrite_blocker', info.rewriteBlocker },
    {
      'tes4',
      if tes4
        then json.object {
          { 'version', tes4.version },
          { 'archive_types', tes4.archiveTypes },
          { 'archive_types_bits', tes4.archiveTypesBits },
          { 'archive_flags', list(tes4.archiveFlags, kebab) },
          { 'archive_flags_bits', tes4.archiveFlagsBits },
          { 'unsupported_archive_flags_bits', tes4.unsupportedArchiveFlagsBits },
          { 'name_mode', kebab(tes4.nameMode) },
        }
        else nil,
    },
    {
      'ba2',
      if ba2
        then json.object {
          { 'version', ba2.version },
          { 'payload_format', ba2.payloadFormat },
          { 'compression_format', ba2.compressionFormat },
          { 'strings', ba2.strings },
        }
        else nil,
    },
  }
end

local function entryShape(entry: tool.ArchiveEntry): json.Value
  return json.object {
    { 'path', entry.path },
    { 'path_bytes_hex', entry.pathBytesHex },
    { 'size', json.optional(entry.size) },
    { 'compressed_size', json.optional(entry.compressedSize) },
  }
end

local function issueShape(issue: tool.PathIssue): json.Value
  return json.object {
    { 'path', issue.path },
    { 'path_bytes_hex', issue.pathBytesHex },
    { 'raw_path_bytes_hex', issue.rawPathBytesHex },
    { 'colliding_raw_path_bytes_hex', issue.collidingRawPathBytesHex },
  }
end

local function verifyShape(report: tool.VerifyReport): json.Value
  return json.object {
    { 'path', report.path },
    { 'format', Formats[report.format] },
    { 'file_count', report.fileCount },
    { 'named_entry_count', report.namedEntryCount },
    { 'unnameable_entries', report.unnameableEntries },
    { 'rewritable', report.rewritable },
    { 'rewrite_blocker', report.rewriteBlocker },
    { 'duplicate_normalized_paths', list(report.duplicateNormalizedPaths, issueShape) },
    { 'unsafe_paths', list(report.unsafePaths, issueShape) },
    { 'payloads_read', json.optional(report.payloadsRead) },
    { 'warnings', names(report.warnings) },
  }
end

--- A payload's fingerprint as the crate writes one: sixteen hexadecimal digits.
local function fingerprint(value: integer?): string?
  if not value then return nil end

  return util.hex64(value)
end

local function stateFields(state: tool.DiffState): { { unknown } }
  return {
    { 'size', json.optional(state.size) },
    { 'compressed_size', json.optional(state.compressedSize) },
    { 'payload_fingerprint', fingerprint(state.payloadFingerprint) },
  }
end

local function diffEntryShape(entry: tool.DiffEntry): json.Value
  return json.object {
    { 'path', entry.path },
    { 'path_bytes_hex', entry.pathBytesHex },
    { 'size', json.optional(entry.size) },
    { 'compressed_size', json.optional(entry.compressedSize) },
    { 'payload_fingerprint', fingerprint(entry.payloadFingerprint) },
  }
end

local function diffChangeShape(change: tool.DiffChange): json.Value
  return json.object {
    { 'path', change.path },
    { 'path_bytes_hex', change.pathBytesHex },
    { 'old', json.object(stateFields(change.old)) },
    { 'new', json.object(stateFields(change.new)) },
  }
end

local function diffShape(report: tool.DiffReport): json.Value
  return json.object {
    { 'old', report.old },
    { 'new', report.new },
    { 'comparison', kebab(report.comparison) },
    { 'fingerprint_payloads', report.fingerprintPayloads },
    { 'added', list(report.added, diffEntryShape) },
    { 'removed', list(report.removed, diffEntryShape) },
    { 'changed', list(report.changed, diffChangeShape) },
    { 'unchanged', report.unchanged },
  }
end

local function extractPlanShape(plan: tool.ExtractPlan): json.Value
  return json.object {
    { 'operation', kebab(plan.operation) },
    { 'archive', plan.archive },
    { 'output', plan.output },
    {
      'entries',
      list(plan.entries, function(row: tool.ExtractPlanRow): json.Value
        return json.object {
          { 'action', row.action },
          { 'path', row.path },
          { 'path_bytes_hex', row.pathBytesHex },
          { 'target', row.target },
        }
      end),
    },
  }
end

local function archivePlanRows(rows: { tool.ArchivePlanRow }): json.Value
  return list(rows, function(row: tool.ArchivePlanRow): json.Value
    return json.object {
      { 'action', row.action },
      { 'source', row.source },
      { 'path', row.path },
      { 'path_bytes_hex', row.pathBytesHex },
      { 'size', row.size },
    }
  end)
end

local function createPlanShape(plan: tool.CreatePlan): json.Value
  return json.object {
    { 'operation', plan.operation },
    { 'format', Formats[plan.format] },
    { 'output', plan.output },
    { 'files', plan.files },
    { 'entries', archivePlanRows(plan.entries) },
  }
end

local function addPlanShape(plan: tool.AddPlan): json.Value
  return json.object {
    { 'operation', plan.operation },
    { 'archive', plan.archive },
    { 'output', plan.output },
    { 'format', Formats[plan.format] },
    { 'files', plan.files },
    { 'added', plan.added },
    { 'replaced', plan.replaced },
    { 'preserved', plan.preserved },
    { 'entries', archivePlanRows(plan.entries) },
  }
end

local function lines(rows: { string }): string
  if #rows == 0 then return '' end

  return `{table.concat(rows, '\n')}\n`
end

local function listText(entries: { tool.ArchiveEntry }, long: boolean?): string
  if not long then return lines([for entry in entries => entry.path]) end

  return lines([for {path, size} in entries => string.format(
    '%10s %s',
    if size then string.format('%d', size) else '-',
    path
  )])
end

local function verifyText(report: tool.VerifyReport): string
  local rows = {
    `format: {Formats[report.format]}`,
    `files: {report.fileCount}`,
    `named: {report.namedEntryCount}`,
    `unnameable: {report.unnameableEntries}`,
    `rewritable: {report.rewritable}`,
  }

  if report.rewriteBlocker then table.insert(rows, `rewrite blocker: {report.rewriteBlocker}`) end

  if report.payloadsRead then table.insert(rows, `payloads read: {report.payloadsRead}`) end

  for _, warning in report.warnings do
    table.insert(rows, `warning: {warning}`)
  end

  return lines(rows)
end

local function diffText(report: tool.DiffReport): string
  return lines {
    `comparison: {kebab(report.comparison)}`,
    `added: {#report.added}`,
    `removed: {#report.removed}`,
    `changed: {#report.changed}`,
    `unchanged: {report.unchanged}`,
  }
end

--- An extraction's counts; nothing for one that didn't run, which wrote nothing to say.
local function summary(counts: tool.Summary?, options: Options): string
  if not counts then return '' end

  if options.json then
    return document(
      json.object { { 'extracted', counts.extracted }, { 'skipped', counts.skipped } }
    )
  end

  if counts.skipped > 0 then
    return lines { `extracted: {counts.extracted}`, `skipped: {counts.skipped}` }
  end

  return lines { `extracted: {counts.extracted}` }
end

--- How many files an archive written holds; nothing for one that wasn't written.
local function count(files: number?, options: Options): string
  if not files then return '' end

  if options.json then return document(json.object { { 'files', files } }) end

  return lines { `files: {files}` }
end

--- The text dream_archivetool writes to standard output for `result`: with `options.json` its
--- JSON, and a dry run's plan as JSON either way.
local function render(result: commands.Result, given: Options?): string
  local options: Options = given or {}
  if result.command == 'info' then
    if options.json then return document(infoShape(result.info)) end

    return lines { `format: {Formats[result.info.format]}`, `files: {result.info.fileCount}` }
  end

  if result.command == 'list' then
    if options.json then return document(list(result.entries, entryShape)) end

    return listText(result.entries, options.long)
  end

  if result.command == 'verify' then
    return if options.json then document(verifyShape(result.report)) else verifyText(result.report)
  end

  if result.command == 'diff' then
    return if options.json then document(diffShape(result.report)) else diffText(result.report)
  end

  if result.command == 'extract' then
    if result.data then return result.data end

    if result.dryRun and result.plan then return document(extractPlanShape(result.plan)) end

    return summary(result.summary, options)
  end

  if result.command == 'extractAll' then
    if result.dryRun then return document(extractPlanShape(result.plan)) end

    return summary(result.summary, options)
  end

  if result.command == 'create' then
    if result.dryRun then return document(createPlanShape(result.plan)) end

    return count(result.files, options)
  end

  if result.dryRun then return document(addPlanShape(result.plan)) end

  return count(result.files, options)
end

return table.freeze { Formats = Formats, kebab = kebab, render = render }
