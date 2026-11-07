export interface UiPreferences {
  theme: 'dark' | 'light' | 'system';
  fontSize: number;
  terminalFontSize: number;
  terminalFontFamily: string;
  terminalLineHeight: number;
  cursorStyle: 'block' | 'bar' | 'underline';
  cursorBlink: boolean;
  paneTitles: boolean;
  reduceMotion: boolean;
  sidebarWidth: number;
  rightSidebarWidth: number;
}

export const defaultPreferences: UiPreferences = {
  theme: 'dark',
  fontSize: 14,
  terminalFontSize: 14,
  terminalFontFamily: '"Cascadia Mono", "Orca Nerd Font Symbols", Consolas, monospace',
  terminalLineHeight: 1.2,
  cursorStyle: 'block',
  cursorBlink: true,
  paneTitles: false,
  reduceMotion: false,
  sidebarWidth: 280,
  rightSidebarWidth: 350,
};

export const preferenceKey = 'minnow.orca.appearance';

export function loadPreferences(): UiPreferences {
  try {
    const value = JSON.parse(localStorage.getItem(preferenceKey) ?? '{}');
    if (!value || typeof value !== 'object') return { ...defaultPreferences };
    return {
      theme: ['dark', 'light', 'system'].includes(value.theme) ? value.theme : defaultPreferences.theme,
      fontSize: bounded(value.fontSize, 11, 20, defaultPreferences.fontSize),
      terminalFontSize: bounded(value.terminalFontSize, 8, 32, defaultPreferences.terminalFontSize),
      terminalFontFamily: typeof value.terminalFontFamily === 'string' && value.terminalFontFamily.trim() ? value.terminalFontFamily : defaultPreferences.terminalFontFamily,
      terminalLineHeight: bounded(value.terminalLineHeight, 1, 2, defaultPreferences.terminalLineHeight),
      cursorStyle: ['block', 'bar', 'underline'].includes(value.cursorStyle) ? value.cursorStyle : defaultPreferences.cursorStyle,
      cursorBlink: typeof value.cursorBlink === 'boolean' ? value.cursorBlink : defaultPreferences.cursorBlink,
      paneTitles: typeof value.paneTitles === 'boolean' ? value.paneTitles : defaultPreferences.paneTitles,
      reduceMotion: typeof value.reduceMotion === 'boolean' ? value.reduceMotion : defaultPreferences.reduceMotion,
      sidebarWidth: bounded(value.sidebarWidth, 220, 500, defaultPreferences.sidebarWidth),
      rightSidebarWidth: bounded(value.rightSidebarWidth, 200, 600, defaultPreferences.rightSidebarWidth),
    };
  } catch {
    return { ...defaultPreferences };
  }
}

function bounded(value: unknown, min: number, max: number, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback;
}
