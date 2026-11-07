// Ported from Orca 1.4.214 components/settings/search/settings-search.ts (MIT).
export interface SearchEntry {
  title: string;
  description?: string;
  keywords?: string[];
}

const QUERY_MAX_BYTES = 2 * 1024;
const NO_MATCH = 0;
const EMPTY_QUERY = 1;

const PANE_TITLE_SCORE = { exact: 900, prefix: 850, substring: 800 };
const ENTRY_TITLE_SCORE = { exact: 700, prefix: 650, substring: 600 };
const DESCRIPTION_SCORE = { exact: 500, prefix: 450, substring: 400 };
const KEYWORD_SCORE = { exact: 300, prefix: 250, substring: 200 };

function fold(value: string): string {
  // Turkish İ lowercases to i + U+0307; drop the dot so plain-i queries still match.
  return value.toLowerCase().replaceAll('\u0307', '');
}

export function normalizeQuery(query: string): string {
  return fold(query.trim());
}

export function isQueryTooLarge(query: string): boolean {
  return new TextEncoder().encode(query).length > QUERY_MAX_BYTES;
}

type Tier = { exact: number; prefix: number; substring: number };

function scoreText(query: string, value: string | undefined, tier: Tier): number {
  if (!value) return NO_MATCH;
  const normalized = fold(value);
  if (normalized === query) return tier.exact;
  if (normalized.startsWith(query)) return tier.prefix;
  if (normalized.includes(query)) return tier.substring;
  return NO_MATCH;
}

function scoreValues(query: string, values: readonly string[] | undefined, tier: Tier): number {
  return (values ?? []).reduce((best, value) => Math.max(best, scoreText(query, value, tier)), NO_MATCH);
}

/** Scores one entry list using Orca's tier order; the first entry is treated as the pane title. */
export function scoreSearch(query: string, entries: SearchEntry | readonly SearchEntry[]): number {
  if (isQueryTooLarge(query)) return NO_MATCH;
  const normalized = normalizeQuery(query);
  if (!normalized) return EMPTY_QUERY;
  const list = Array.isArray(entries) ? entries : [entries as SearchEntry];
  return list.reduce((best, entry, index) => {
    const titleTier = index === 0 ? PANE_TITLE_SCORE : ENTRY_TITLE_SCORE;
    return Math.max(
      best,
      scoreText(normalized, entry.title, titleTier),
      scoreText(normalized, entry.description, DESCRIPTION_SCORE),
      scoreValues(normalized, entry.keywords, KEYWORD_SCORE)
    );
  }, NO_MATCH);
}

export function matchesSearch(query: string, entries: SearchEntry | readonly SearchEntry[]): boolean {
  return scoreSearch(query, entries) > NO_MATCH;
}

export interface Rankable {
  id: string;
  title: string;
  description: string;
  keywords: string[];
}

/** Ranks sections by score, then by their original order. Empty queries keep source order. */
export function rankSections(query: string, sections: readonly Rankable[]): Rankable[] {
  if (isQueryTooLarge(query)) return [];
  if (!normalizeQuery(query)) return [...sections];
  return sections
    .map((section, index) => ({
      section,
      index,
      score: scoreSearch(query, {
        title: section.title,
        description: section.description,
        keywords: section.keywords
      })
    }))
    .filter((candidate) => candidate.score > NO_MATCH)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((candidate) => candidate.section);
}
