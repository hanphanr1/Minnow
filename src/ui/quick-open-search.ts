// Fuzzy path ranking ported from Orca's shared/quick-open-path-search.ts (MIT): every
// whitespace-separated term must match as a greedy subsequence of the path; lower
// scores win, filename hits and path-segment starts are rewarded.

export const QUICK_OPEN_RESULT_LIMIT = 50;
export const QUICK_OPEN_QUERY_MAX_BYTES = 2 * 1024;

export interface QuickOpenFile {
  path: string;
  lowerPath: string;
  lowerFilename: string;
  index: number;
}

export interface QuickOpenResult {
  path: string;
  score: number;
}

export function prepareQuickOpenFiles(paths: readonly string[]): QuickOpenFile[] {
  return paths.map((path, index) => {
    const normalized = path.replace(/\\/g, '/');
    const slash = normalized.lastIndexOf('/');
    return {
      path,
      lowerPath: normalized.toLowerCase(),
      lowerFilename: normalized.slice(slash + 1).toLowerCase(),
      index
    };
  });
}

function fuzzyMatch(query: string, file: QuickOpenFile): number | null {
  let queryIndex = 0;
  let score = 0;
  let last = -1;
  while (queryIndex < query.length) {
    const from = last === -1 ? 0 : last + 1;
    const found =
      file.lowerPath[from] === query[queryIndex]
        ? from
        : file.lowerPath.indexOf(query[queryIndex], from + 1);
    if (found === -1) return null;
    score += last === -1 ? 0 : found - last - 1;
    if (found > 0 && '/.-'.includes(file.lowerPath[found - 1])) score -= 5;
    last = found;
    queryIndex += 1;
  }
  if (file.lowerFilename.includes(query)) score -= 100;
  return score;
}

function compareNames(left: string, right: string): number {
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: 'base' });
}

export function rankQuickOpenFiles(
  query: string,
  files: readonly QuickOpenFile[],
  limit = QUICK_OPEN_RESULT_LIMIT
): QuickOpenResult[] {
  if (limit <= 0 || new TextEncoder().encode(query).length > QUICK_OPEN_QUERY_MAX_BYTES) return [];
  const terms = [
    ...new Set(query.trim().replace(/\\/g, '/').toLowerCase().split(/\s+/).filter(Boolean))
  ].sort();
  const ranked: Array<QuickOpenResult & { index: number }> = [];
  for (const file of files) {
    let total = 0;
    let matched = true;
    for (const term of terms) {
      const value = fuzzyMatch(term, file);
      if (value === null) {
        matched = false;
        break;
      }
      total += value;
    }
    if (matched) ranked.push({ path: file.path, score: total, index: file.index });
  }
  ranked.sort(
    (left, right) =>
      left.score - right.score || compareNames(left.path, right.path) || left.index - right.index
  );
  return ranked.slice(0, limit).map(({ path, score }) => ({ path, score }));
}

/** `path:12` and `path:12:5` open at a line, like Orca's quick-open query target. */
export function parseQuickOpenTarget(query: string): { pathQuery: string; line?: number } {
  const match = /^(.*?):(\d+)(?::\d+)?$/.exec(query.trim());
  if (!match || !match[1]) return { pathQuery: query.trim() };
  const line = Number(match[2]);
  return line > 0 ? { pathQuery: match[1], line } : { pathQuery: query.trim() };
}
