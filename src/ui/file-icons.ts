import {
  File,
  FileArchive,
  FileAudio,
  FileCode2,
  FileCog,
  FileImage,
  FileJson,
  FileText,
  FileVideo,
  Lock
} from 'lucide-svelte';

export type FileIconComponent = typeof File;

const CODE_EXTENSIONS =
  /\.(ts|tsx|js|jsx|mjs|cjs|rs|css|scss|sass|less|html|svelte|vue|py|go|java|kt|kts|c|h|cc|cpp|hpp|cs|rb|php|sh|bash|zsh|fish|ps1|lua|swift|dart|sql|toml|ya?ml|xml|graphql|gql)$/i;
const TEXT_EXTENSIONS = /\.(md|mdx|txt|rst|log|csv|tsv)$/i;
const IMAGE_EXTENSIONS = /\.(png|jpe?g|gif|webp|ico|bmp|avif|svg)$/i;
const JSON_EXTENSIONS = /\.(json|jsonc|json5)$/i;
const ARCHIVE_EXTENSIONS = /\.(zip|tar|gz|tgz|bz2|xz|7z|rar|zst)$/i;
const AUDIO_EXTENSIONS = /\.(mp3|wav|ogg|flac|aac|m4a|opus)$/i;
const VIDEO_EXTENSIONS = /\.(mp4|mov|mkv|webm|avi|m4v)$/i;
const SECRET_EXTENSIONS = /\.(pem|key|crt|cer|p12|pfx|p8)$/i;
const CONFIG_EXTENSIONS = /\.(ini|conf|cfg|properties|editorconfig)$/i;

/** Lucide icon matching Orca's per-extension file type glyphs. */
export function fileIconFor(path: string): FileIconComponent {
  const name = path.toLowerCase();
  if (IMAGE_EXTENSIONS.test(name)) return FileImage;
  if (JSON_EXTENSIONS.test(name)) return FileJson;
  if (CODE_EXTENSIONS.test(name)) return FileCode2;
  if (TEXT_EXTENSIONS.test(name)) return FileText;
  if (ARCHIVE_EXTENSIONS.test(name)) return FileArchive;
  if (AUDIO_EXTENSIONS.test(name)) return FileAudio;
  if (VIDEO_EXTENSIONS.test(name)) return FileVideo;
  if (
    SECRET_EXTENSIONS.test(name) ||
    /(^|\/)(\.env(\..+)?|id_rsa|id_ed25519|\.npmrc|\.pypirc)$/i.test(name)
  ) {
    return Lock;
  }
  if (CONFIG_EXTENSIONS.test(name)) return FileCog;
  return File;
}

export function normalizeWorkspacePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/^\.\//, '').replace(/\/+$/, '');
}

export function joinWorkspacePath(parent: string, name: string): string {
  const cleanParent = normalizeWorkspacePath(parent);
  const cleanName = normalizeWorkspacePath(name);
  if (!cleanParent) return cleanName;
  if (!cleanName) return cleanParent;
  return `${cleanParent}/${cleanName}`;
}

export function baseName(path: string): string {
  const normalized = normalizeWorkspacePath(path);
  const separator = normalized.lastIndexOf('/');
  return separator < 0 ? normalized : normalized.slice(separator + 1);
}

export function dirName(path: string): string {
  const normalized = normalizeWorkspacePath(path);
  const separator = normalized.lastIndexOf('/');
  return separator < 0 ? '' : normalized.slice(0, separator);
}

export type GitStatusTone =
  | 'added'
  | 'modified'
  | 'deleted'
  | 'renamed'
  | 'copied'
  | 'untracked'
  | 'conflict';

export interface GitStatusDescription {
  letter: string;
  tone: GitStatusTone;
  color: string;
  label: string;
}

const CONFLICT_CODES: Record<string, true> = {
  U: true,
  UU: true,
  AA: true,
  AU: true,
  UA: true,
  DU: true,
  UD: true,
  DD: true
};

/**
 * Maps the backend's Git status text (porcelain code or long name) to Orca's
 * single-letter decoration and its themed colour. Unknown values stay visible
 * as their first character instead of being presented as a successful state.
 */
export function describeGitStatus(status: string): GitStatusDescription {
  const raw = status.trim();
  const upper = raw.toUpperCase();
  const lower = raw.toLowerCase();
  if (CONFLICT_CODES[upper] || lower.includes('conflict')) {
    return {
      letter: '!',
      tone: 'conflict',
      color: 'var(--git-decoration-deleted)',
      label: 'Conflict'
    };
  }
  if (lower === '?' || lower === 'untracked' || lower === 'new') {
    return {
      letter: 'U',
      tone: 'untracked',
      color: 'var(--git-decoration-untracked)',
      label: 'Untracked'
    };
  }
  if (lower.includes('rename') || upper.startsWith('R')) {
    return {
      letter: 'R',
      tone: 'renamed',
      color: 'var(--git-decoration-renamed)',
      label: 'Renamed'
    };
  }
  if (lower.includes('cop') || upper.startsWith('C')) {
    return {
      letter: 'C',
      tone: 'copied',
      color: 'var(--git-decoration-copied)',
      label: 'Copied'
    };
  }
  if (lower.includes('delet') || upper.startsWith('D')) {
    return {
      letter: 'D',
      tone: 'deleted',
      color: 'var(--git-decoration-deleted)',
      label: 'Deleted'
    };
  }
  if (lower.includes('add') || upper.startsWith('A')) {
    return {
      letter: 'A',
      tone: 'added',
      color: 'var(--git-decoration-added)',
      label: 'Added'
    };
  }
  if (upper === 'T' || lower.includes('type')) {
    return {
      letter: 'T',
      tone: 'modified',
      color: 'var(--git-decoration-modified)',
      label: 'Type Changed'
    };
  }
  if (lower.includes('modif') || upper.startsWith('M')) {
    return {
      letter: 'M',
      tone: 'modified',
      color: 'var(--git-decoration-modified)',
      label: 'Modified'
    };
  }
  return {
    letter: upper.charAt(0) || '?',
    tone: 'modified',
    color: 'var(--git-decoration-modified)',
    label: raw || 'Changed'
  };
}

