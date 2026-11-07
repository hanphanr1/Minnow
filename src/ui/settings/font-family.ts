// Ported from Orca 1.4.214 lib/monospace-font-family.ts (MIT, © 2026 Lovecast Inc.).
// The bundled Nerd Symbols face covers PUA glyphs (U+E000–U+F8FF) that plain mono fonts lack,
// so the stored terminal family is a full fallback chain rather than a single name.
export const BUNDLED_SYMBOLS_FONT_FAMILY = 'Orca Nerd Font Symbols';

const FALLBACK_FONTS = [
  'SF Mono',
  'Menlo',
  'Monaco',
  'Cascadia Mono',
  'Consolas',
  'DejaVu Sans Mono',
  'Liberation Mono',
  BUNDLED_SYMBOLS_FONT_FAMILY,
  'Symbols Nerd Font Mono',
  'MesloLGS Nerd Font',
  'JetBrainsMono Nerd Font',
  'Hack Nerd Font',
  'monospace'
] as const;

const GENERIC_FAMILIES: Record<string, true> = {
  monospace: true,
  'sans-serif': true,
  serif: true,
  'system-ui': true,
  'ui-monospace': true,
  cursive: true,
  fantasy: true
};

/** Reads the editable primary family back out of a stored CSS font stack. */
export function firstFontFamily(stack: string): string {
  const first = stack.split(',')[0]?.trim() ?? '';
  return first.replace(/^["']/, '').replace(/["']$/, '').trim();
}

export function buildTerminalFontFamily(fontFamily: string): string {
  const primary = firstFontFamily(fontFamily);
  const parts: string[] = [];
  const seen: string[] = [];

  if (primary) {
    const lower = primary.toLowerCase();
    parts.push(GENERIC_FAMILIES[lower] ? lower : `"${primary}"`);
    seen.push(lower);
  }

  for (const fallback of FALLBACK_FONTS) {
    const lower = fallback.toLowerCase();
    if (seen.some((value) => value.includes(lower))) continue;
    parts.push(GENERIC_FAMILIES[lower] ? lower : `"${fallback}"`);
    seen.push(lower);
  }

  return parts.join(', ');
}
