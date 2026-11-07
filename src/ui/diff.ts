/**
 * Parser for the unified patch returned by the Minnow `diff_worker` command.
 *
 * The backend runs `git diff --no-color --no-ext-diff <base> --` and appends
 * `git diff --no-index /dev/null <untracked>` patches, so the input is a
 * standard git patch (possibly with quoted octal paths). This module only
 * reports what the patch contains: it never fills in file contents, staging
 * areas, commits or provider data that git did not print.
 */

export type DiffFileStatus = 'modified' | 'added' | 'deleted' | 'renamed' | 'copied'
export type DiffLineKind = 'context' | 'add' | 'remove'

export interface DiffLine {
  kind: DiffLineKind
  text: string
  oldNumber: number | null
  newNumber: number | null
  /** The next line was `\ No newline at end of file`. */
  noNewline: boolean
}

export interface DiffHunk {
  /** The raw `@@ ... @@` line without its trailing newline. */
  header: string
  oldStart: number
  oldCount: number
  newStart: number
  newCount: number
  section: string
  lines: DiffLine[]
}

export interface DiffFile {
  /** New path, or the old path for a deletion. */
  path: string
  /** Previous path for renames and copies. */
  oldPath: string | null
  status: DiffFileStatus
  binary: boolean
  /** A mode change with no content hunks. */
  modeOnly: boolean
  oldMode: string | null
  newMode: string | null
  hunks: DiffHunk[]
  added: number
  removed: number
}

export interface ParsedDiff {
  files: DiffFile[]
  added: number
  removed: number
  binaryFiles: number
}

const DEV_NULL = '/dev/null'
const HUNK_HEADER =
  /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(?: ?(.*))?$/

const C_ESCAPES: Record<string, string> = {
  a: '\x07',
  b: '\b',
  t: '\t',
  n: '\n',
  v: '\v',
  f: '\f',
  r: '\r',
  '"': '"',
  '\\': '\\',
}

/** Decodes git's C-style quoted path (`"a/\303\274..."`). */
export function unquoteGitPath(value: string): string {
  if (value.length < 2 || value[0] !== '"' || value[value.length - 1] !== '"') {
    return value
  }
  return decodeEscapedPath(value.slice(1, -1))
}

function decodeEscapedPath(body: string): string {
  const encoder = new TextEncoder()
  const bytes: number[] = []
  for (let index = 0; index < body.length; index += 1) {
    const character = body[index]
    if (character !== '\\') {
      bytes.push(...encoder.encode(character))
      continue
    }
    index += 1
    if (index >= body.length) {
      bytes.push(0x5c)
      break
    }
    const escaped = body[index]
    const simple = C_ESCAPES[escaped]
    if (simple !== undefined) {
      bytes.push(...encoder.encode(simple))
      continue
    }
    if (escaped >= '0' && escaped <= '7') {
      let octal = escaped
      while (
        octal.length < 3 &&
        index + 1 < body.length &&
        body[index + 1] >= '0' &&
        body[index + 1] <= '7'
      ) {
        index += 1
        octal += body[index]
      }
      bytes.push(Number.parseInt(octal, 8) & 0xff)
      continue
    }
    bytes.push(...encoder.encode(escaped))
  }
  return new TextDecoder('utf-8', { fatal: false }).decode(new Uint8Array(bytes))
}

function stripDiffPrefix(value: string, prefix: 'a' | 'b'): string {
  const path = unquoteGitPath(value)
  if (path === DEV_NULL) return path
  return path.startsWith(`${prefix}/`) ? path.slice(2) : path
}

/** Reads a possibly quoted token; returns the token and the remaining text. */
function readQuotedToken(source: string): { value: string; rest: string } | null {
  const input = source.replace(/^\s+/, '')
  if (!input.startsWith('"')) return null
  let index = 1
  while (index < input.length) {
    const character = input[index]
    if (character === '\\') {
      index += 2
      continue
    }
    if (character === '"') break
    index += 1
  }
  if (index >= input.length) return null
  return {
    value: input.slice(0, index + 1),
    rest: input.slice(index + 1).replace(/^\s+/, ''),
  }
}

function parseDiffGitLine(rest: string): { oldPath: string | null; newPath: string | null } {
  const input = rest.replace(/^\s+/, '')
  let first: string
  let remaining: string
  if (input.startsWith('"')) {
    const token = readQuotedToken(input)
    if (!token) return { oldPath: null, newPath: null }
    first = token.value
    remaining = token.rest
  } else {
    // Paths with spaces are not quoted by git; the second path always starts
    // with `b/`, so prefer that boundary over the first whitespace.
    const boundary = input.startsWith('a/') ? input.indexOf(' b/') : -1
    const separator = boundary > 2 ? boundary : input.indexOf(' ')
    if (separator < 0) {
      first = input
      remaining = ''
    } else {
      first = input.slice(0, separator)
      remaining = input.slice(separator + 1).replace(/^\s+/, '')
    }
  }
  const second = remaining.startsWith('"')
    ? (readQuotedToken(remaining)?.value ?? '')
    : remaining
  const oldPath = stripDiffPrefix(first, 'a')
  const newPath = second ? stripDiffPrefix(second, 'b') : null
  return {
    oldPath: oldPath === DEV_NULL ? null : oldPath,
    newPath: newPath === DEV_NULL ? null : newPath,
  }
}

function parsePatchPath(line: string, prefix: 'a' | 'b'): string | null {
  let value = line.slice(4)
  if (value.startsWith('"')) {
    const token = readQuotedToken(value)
    if (!token) return null
    value = token.value
  } else {
    // `--- a/file\t` (empty timestamp) and `--- a/file\t2026-...` both use a tab.
    const tab = value.indexOf('\t')
    if (tab >= 0) value = value.slice(0, tab)
  }
  const path = stripDiffPrefix(value, prefix)
  return path === DEV_NULL ? null : path
}

function parseBinaryLine(line: string): { oldPath: string | null; newPath: string | null } {
  const body = line.slice('Binary files '.length).replace(/ differ$/, '')
  let oldValue = ''
  let newValue = ''
  if (body.startsWith('"')) {
    const token = readQuotedToken(body)
    if (token) {
      oldValue = token.value
      newValue = token.rest.replace(/^and\s+/, '')
    }
  } else {
    const separator = body.indexOf(' and ')
    if (separator >= 0) {
      oldValue = body.slice(0, separator)
      newValue = body.slice(separator + 5)
    } else {
      oldValue = body
    }
  }
  const oldPath = oldValue ? stripDiffPrefix(oldValue, 'a') : null
  const newPath = newValue ? stripDiffPrefix(newValue, 'b') : null
  return {
    oldPath: oldPath === DEV_NULL ? null : oldPath,
    newPath: newPath === DEV_NULL ? null : newPath,
  }
}

function parseHunkHeader(line: string): DiffHunk | null {
  const match = HUNK_HEADER.exec(line)
  if (!match) return null
  return {
    header: line.replace(/\s+$/, ''),
    oldStart: Number(match[1]),
    oldCount: match[2] === undefined ? 1 : Number(match[2]),
    newStart: Number(match[3]),
    newCount: match[4] === undefined ? 1 : Number(match[4]),
    section: (match[5] ?? '').trim(),
    lines: [],
  }
}

export function parseUnifiedDiff(text: string): ParsedDiff {
  const files: DiffFile[] = []
  let file: DiffFile | null = null
  let hunk: DiffHunk | null = null
  let headerOldPath: string | null = null
  let headerNewPath: string | null = null
  let oldNumber = 0
  let newNumber = 0
  let oldLeft = 0
  let newLeft = 0
  let lastLine: DiffLine | null = null

  function startFile(line: string): void {
    const header = parseDiffGitLine(line.slice('diff --git '.length))
    headerOldPath = header.oldPath
    headerNewPath = header.newPath
    file = {
      path: '',
      oldPath: null,
      status: 'modified',
      binary: false,
      modeOnly: false,
      oldMode: null,
      newMode: null,
      hunks: [],
      added: 0,
      removed: 0,
    }
    hunk = null
    lastLine = null
    oldLeft = 0
    newLeft = 0
  }

  function finishFile(): void {
    if (!file) return
    const finished = file
    let oldPath = finished.oldPath ?? headerOldPath
    let newPath = finished.path || headerNewPath
    if (finished.status === 'added') {
      finished.path = newPath ?? oldPath ?? ''
      finished.oldPath = null
    } else if (finished.status === 'deleted') {
      finished.path = oldPath ?? newPath ?? ''
      finished.oldPath = null
    } else if (finished.status === 'renamed' || finished.status === 'copied') {
      finished.path = newPath ?? oldPath ?? ''
      finished.oldPath = oldPath && oldPath !== finished.path ? oldPath : null
    } else {
      finished.path = newPath ?? oldPath ?? ''
      finished.oldPath = null
    }
    finished.modeOnly =
      finished.status === 'modified' &&
      finished.hunks.length === 0 &&
      !finished.binary &&
      finished.oldMode !== null &&
      finished.newMode !== null
    if (finished.path) files.push(finished)
    file = null
    hunk = null
    lastLine = null
    headerOldPath = null
    headerNewPath = null
    oldLeft = 0
    newLeft = 0
  }

  function startHunk(line: string): void {
    const parsed = parseHunkHeader(line)
    if (!parsed || !file) return
    file.hunks.push(parsed)
    hunk = parsed
    lastLine = null
    oldNumber = parsed.oldStart
    newNumber = parsed.newStart
    oldLeft = parsed.oldCount
    newLeft = parsed.newCount
  }

  function pushLine(kind: DiffLineKind, textValue: string): void {
    if (!file || !hunk) return
    const line: DiffLine = {
      kind,
      text: textValue,
      oldNumber: kind === 'add' ? null : oldNumber,
      newNumber: kind === 'remove' ? null : newNumber,
      noNewline: false,
    }
    hunk.lines.push(line)
    lastLine = line
    if (kind === 'add') {
      newNumber += 1
      newLeft = Math.max(0, newLeft - 1)
      file.added += 1
    } else if (kind === 'remove') {
      oldNumber += 1
      oldLeft = Math.max(0, oldLeft - 1)
      file.removed += 1
    } else {
      oldNumber += 1
      newNumber += 1
      oldLeft = Math.max(0, oldLeft - 1)
      newLeft = Math.max(0, newLeft - 1)
    }
  }

  function markNoNewline(): void {
    if (lastLine) lastLine.noNewline = true
  }

  function consumeHeaderLine(line: string): void {
    if (!file) return
    if (line.startsWith('--- ')) {
      if (line.startsWith('--- /dev/null') || line.startsWith('--- "/dev/null"')) {
        file.status = 'added'
        file.oldPath = null
      } else {
        const path = parsePatchPath(line, 'a')
        if (path !== null) file.oldPath = path
      }
      return
    }
    if (line.startsWith('+++ ')) {
      if (line.startsWith('+++ /dev/null') || line.startsWith('+++ "/dev/null"')) {
        file.status = 'deleted'
        file.path = ''
      } else {
        const path = parsePatchPath(line, 'b')
        if (path !== null) file.path = path
      }
      return
    }
    if (line.startsWith('new file mode ')) {
      file.status = 'added'
      file.newMode = line.slice('new file mode '.length).trim()
      return
    }
    if (line.startsWith('deleted file mode ')) {
      file.status = 'deleted'
      file.oldMode = line.slice('deleted file mode '.length).trim()
      return
    }
    if (line.startsWith('old mode ')) {
      file.oldMode = line.slice('old mode '.length).trim()
      return
    }
    if (line.startsWith('new mode ')) {
      file.newMode = line.slice('new mode '.length).trim()
      return
    }
    if (line.startsWith('rename from ')) {
      file.status = 'renamed'
      file.oldPath = unquoteGitPath(line.slice('rename from '.length))
      return
    }
    if (line.startsWith('rename to ')) {
      file.status = 'renamed'
      file.path = unquoteGitPath(line.slice('rename to '.length))
      return
    }
    if (line.startsWith('copy from ')) {
      file.status = 'copied'
      file.oldPath = unquoteGitPath(line.slice('copy from '.length))
      return
    }
    if (line.startsWith('copy to ')) {
      file.status = 'copied'
      file.path = unquoteGitPath(line.slice('copy to '.length))
      return
    }
    if (line.startsWith('Binary files ')) {
      file.binary = true
      const paths = parseBinaryLine(line)
      if (paths.oldPath !== null) {
        file.oldPath = paths.oldPath
        if (!file.path) file.path = paths.oldPath
      }
      if (paths.newPath !== null) file.path = paths.newPath
      return
    }
    if (line === 'GIT binary patch') {
      file.binary = true
    }
  }

  const lines = text.split('\n')
  for (const line of lines) {
    if (
      line.startsWith('diff --git ') ||
      line.startsWith('diff --cc ') ||
      line.startsWith('diff --combined ')
    ) {
      finishFile()
      startFile(line)
      continue
    }
    if (!file) continue
    if (line.startsWith('\\ ')) {
      markNoNewline()
      continue
    }
    if (line.startsWith('@@')) {
      startHunk(line)
      continue
    }
    if (hunk && (oldLeft > 0 || newLeft > 0)) {
      const marker = line[0]
      if (marker === '+' || marker === '-' || marker === ' ' || line === '') {
        pushLine(
          marker === '+' ? 'add' : marker === '-' ? 'remove' : 'context',
          line === '' ? '' : line.slice(1),
        )
        continue
      }
      // A malformed/truncated hunk: stop counting it and treat this as file metadata.
      hunk = null
      oldLeft = 0
      newLeft = 0
    }
    consumeHeaderLine(line)
  }
  finishFile()

  let added = 0
  let removed = 0
  let binaryFiles = 0
  for (const parsed of files) {
    added += parsed.added
    removed += parsed.removed
    if (parsed.binary) binaryFiles += 1
  }
  return { files, added, removed, binaryFiles }
}

/**
 * Pairs the lines of one hunk for a side-by-side view. Context lines occupy
 * both columns; consecutive removals and additions are paired by position.
 * A side stays empty when the hunk has no line for it, so nothing outside the
 * patch is invented.
 */
export interface DiffSideBySideRow {
  oldLine: DiffLine | null
  newLine: DiffLine | null
}

export function pairHunkLines(lines: DiffLine[]): DiffSideBySideRow[] {
  const rows: DiffSideBySideRow[] = []
  let index = 0
  while (index < lines.length) {
    const line = lines[index]
    if (line.kind === 'context') {
      rows.push({ oldLine: line, newLine: line })
      index += 1
      continue
    }
    const removals: DiffLine[] = []
    const additions: DiffLine[] = []
    while (index < lines.length && lines[index].kind !== 'context') {
      const changed = lines[index]
      if (changed.kind === 'remove') removals.push(changed)
      else additions.push(changed)
      index += 1
    }
    const paired = Math.max(removals.length, additions.length)
    for (let pairIndex = 0; pairIndex < paired; pairIndex += 1) {
      rows.push({
        oldLine: removals[pairIndex] ?? null,
        newLine: additions[pairIndex] ?? null,
      })
    }
  }
  return rows
}

/** Single-letter status used by the source-control list and diff headers. */
export function diffStatusLetter(status: DiffFileStatus): string {
  switch (status) {
    case 'added':
      return 'A'
    case 'deleted':
      return 'D'
    case 'renamed':
      return 'R'
    case 'copied':
      return 'C'
    default:
      return 'M'
  }
}

/** Theme variable for a status, matching Orca's git decoration colors. */
export function diffStatusColor(status: DiffFileStatus): string {
  return `var(--git-decoration-${status})`
}

/** Splits a path into a dimmed directory prefix and a foreground file name. */
export function splitDiffPath(path: string): { prefix: string; fileName: string } {
  const separator = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  if (separator < 0) return { prefix: '', fileName: path }
  return { prefix: path.slice(0, separator + 1), fileName: path.slice(separator + 1) }
}
