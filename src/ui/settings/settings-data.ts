// Navigation taxonomy ported from Orca 1.4.214:
// hooks/useSettingsNavigationMetadata.ts and hooks/settings-navigation-*-sections.ts
// at e507dd7732139e302f6adaa2917d8e0aee5b1362. Lucide icon choices stay source-matched
// except the custom Orca mark, which becomes SquareCheckBig because no logo asset exists here.
import {
  BarChart3,
  Bell,
  Blocks,
  BookOpen,
  Bot,
  Cable,
  CalendarClock,
  CircleUserRound,
  Files,
  FlaskConical,
  GitBranch,
  Globe,
  History,
  Keyboard,
  ListChecks,
  Lock,
  MessageSquare,
  Mic,
  MousePointerClick,
  Network,
  Palette,
  PanelsTopLeft,
  Play,
  Server,
  SlidersHorizontal,
  Smartphone,
  SquareCheckBig,
  SquareTerminal,
  TabletSmartphone,
  TextCursorInput,
  UserCog,
  Wrench
} from 'lucide-svelte';

// lucide-svelte 0.577 still exports legacy class components, not Svelte 5 Components.
export type NavIcon = typeof Bot;

export type NavGroupId =
  | 'capabilities'
  | 'setup'
  | 'workflows'
  | 'interface'
  | 'remote'
  | 'security'
  | 'advanced'
  | 'experimental';

export interface NavGroup {
  id: NavGroupId;
  title: string;
}

export interface NavSection {
  id: string;
  group: NavGroupId;
  title: string;
  description: string;
  icon: NavIcon;
  badge?: string;
  keywords: string[];
  /** Real panes render interactive settings; the rest render a labelled unavailable shell. */
  implemented: boolean;
  /** Source pane contents listed by the unavailable shell so the exclusion is explicit. */
  includes?: string[];
}

export const NAV_GROUPS: NavGroup[] = [
  { id: 'capabilities', title: 'AI Capabilities' },
  { id: 'setup', title: 'Set Up' },
  { id: 'workflows', title: 'Workflows' },
  { id: 'interface', title: 'Interface' },
  { id: 'remote', title: 'Remote Hosts' },
  { id: 'security', title: 'Privacy & Security' },
  { id: 'advanced', title: 'Advanced' },
  { id: 'experimental', title: 'Experimental' }
];

export const NAV_SECTIONS: NavSection[] = [
  {
    id: 'agents',
    group: 'capabilities',
    title: 'Agents',
    description: 'Manage AI agents, set a default, and customize commands.',
    icon: Bot,
    keywords: ['agent', 'default agent', 'launch', 'commands', 'codex', 'claude'],
    implemented: false,
    includes: [
      'Default agent and launch commands',
      'Agent runtime and workspace trust',
      'Codex terminal-server isolation'
    ]
  },
  {
    id: 'accounts',
    group: 'capabilities',
    title: 'AI Provider Accounts',
    description:
      'Optional account switching and usage setup for Claude, Codex, Gemini, OpenCode Go, MiniMax, and Grok.',
    icon: UserCog,
    badge: 'Optional',
    keywords: ['account', 'provider', 'claude', 'codex', 'gemini', 'grok', 'usage'],
    implemented: false,
    includes: ['Provider sign-in and account switching', 'Usage and quota display', 'Account removal']
  },
  {
    id: 'orchestration',
    group: 'capabilities',
    title: 'Orchestration',
    description: 'Coordinate multiple coding agents through Orca.',
    icon: Network,
    keywords: ['orchestration', 'workers', 'multi-agent', 'nested worker'],
    implemented: false,
    includes: ['Worker auto-release', 'Nested worker depth', 'Orchestration skill setup']
  },
  {
    id: 'computer-use',
    group: 'capabilities',
    title: 'Computer Use',
    description: 'Enable agents to control any app on your computer.',
    icon: MousePointerClick,
    keywords: ['computer use', 'desktop automation', 'control apps'],
    implemented: false,
    includes: ['Computer use skill setup', 'Screen and input permissions', 'Allowed application policy']
  },
  {
    id: 'voice',
    group: 'capabilities',
    title: 'Voice',
    description: 'Local speech-to-text dictation with on-device models.',
    icon: Mic,
    keywords: ['voice', 'dictation', 'speech', 'microphone', 'transcription'],
    implemented: false,
    includes: ['Speech model download', 'Microphone selection', 'OpenAI transcription key']
  },
  {
    id: 'orca-account',
    group: 'setup',
    title: 'Orca Account',
    description: 'Share work instantly and reach your desktop from Orca Mobile wherever you are.',
    icon: CircleUserRound,
    keywords: ['account', 'sign in', 'mobile', 'pairing', 'share'],
    implemented: false,
    includes: ['Account sign-in and device pairing', 'Orca Mobile access', 'Shared work links']
  },
  {
    id: 'setup-guide',
    group: 'setup',
    title: 'Onboarding checklist',
    description: 'Finish the onboarding checklist for core Orca workflows.',
    icon: SquareCheckBig,
    keywords: ['setup guide', 'onboarding', 'checklist', 'getting started'],
    implemented: false,
    includes: ['Checklist progress', 'Milestone steps', 'Workflow tips']
  },
  {
    id: 'general',
    group: 'setup',
    title: 'General',
    description: 'Workspace defaults, app setup, and maintenance.',
    icon: SlidersHorizontal,
    keywords: [
      'general',
      'omp',
      'path',
      'executable',
      'worker',
      'worker count',
      'count',
      'default',
      'backend',
      'about',
      'license',
      'settings'
    ],
    implemented: true
  },
  {
    id: 'integrations',
    group: 'setup',
    title: 'Integrations',
    description: 'Connect GitHub, GitLab, Linear, and source-hosting services.',
    icon: Blocks,
    keywords: ['integrations', 'github', 'gitlab', 'linear', 'jira', 'source hosting'],
    implemented: false,
    includes: ['GitHub and GitLab connections', 'Linear and Jira project links', 'Provider credentials']
  },
  {
    id: 'mobile',
    group: 'setup',
    title: 'Mobile',
    description: 'Control terminals and agents from your phone.',
    icon: Smartphone,
    keywords: ['mobile', 'phone', 'remote access', 'pairing'],
    implemented: false,
    includes: ['Phone pairing', 'Mobile session access', 'Auto-restore fit options']
  },
  {
    id: 'automations',
    group: 'workflows',
    title: 'Automations',
    description: 'Schedule agent work and choose whether Automations appears in the sidebar.',
    icon: CalendarClock,
    keywords: ['automations', 'schedule', 'agent work', 'sidebar'],
    implemented: false,
    includes: ['Scheduled agent runs', 'Automation sidebar visibility', 'Automation deletion prompts']
  },
  {
    id: 'artifacts',
    group: 'workflows',
    title: 'Artifacts',
    description: 'Share HTML and Markdown files with your team and manage their public links.',
    icon: Files,
    badge: 'Beta',
    keywords: ['artifacts', 'html', 'markdown', 'share', 'public links'],
    implemented: false,
    includes: ['Team sharing', 'Public artifact links', 'Artifact retention']
  },
  {
    id: 'share-skills',
    group: 'workflows',
    title: 'Share Skills',
    description: 'Share your skills with an unlisted link. Anyone who has it can install them.',
    icon: BookOpen,
    badge: 'Beta',
    keywords: ['skills', 'share', 'unlisted link', 'install'],
    implemented: false,
    includes: ['Unlisted skill links', 'Install permissions', 'Shared skill versions']
  },
  {
    id: 'session-history',
    group: 'workflows',
    title: 'Agent Session Search',
    description:
      'Search everything your agents have said and done, on this computer and on any paired Orca server.',
    icon: History,
    keywords: ['session', 'history', 'search', 'transcript', 'agent'],
    implemented: true,
    includes: ['Session indexing', 'Search across paired servers', 'Index maintenance']
  },
  {
    id: 'git',
    group: 'workflows',
    title: 'Git & Source Control',
    description: 'Branch naming, base refs, and Git AI Author.',
    icon: GitBranch,
    keywords: ['git', 'source control', 'branch', 'base ref', 'commit message ai'],
    implemented: false,
    includes: ['Branch prefix and base refs', 'Git AI Author commands', 'Provider API budgets']
  },
  {
    id: 'tasks',
    group: 'workflows',
    title: 'Task Sources',
    description: 'Connect providers, install the Linear skill, and choose what appears in Tasks.',
    icon: ListChecks,
    keywords: ['tasks', 'linear', 'jira', 'github issues', 'task sources'],
    implemented: false,
    includes: ['Task provider setup', 'Linear skill install', 'Task visibility toggles']
  },
  {
    id: 'terminal',
    group: 'workflows',
    title: 'Terminal',
    description: 'Shells, renderer, sessions, and terminal behavior.',
    icon: SquareTerminal,
    keywords: [
      'terminal',
      'shell',
      'renderer',
      'gpu',
      'scrollback',
      'pane',
      'pane titles',
      'cursor',
      'font',
      'font size',
      'line height',
      'theme',
      'themes',
      'warp',
      'yaml',
      'session',
      'pty',
      'ack',
      'webgl',
      'transport'
    ],
    implemented: true
  },
  {
    id: 'quick-commands',
    group: 'workflows',
    title: 'Quick Commands',
    description: 'Saved terminal commands, scoped globally or per project.',
    icon: Play,
    keywords: ['quick commands', 'saved commands', 'terminal commands'],
    implemented: true,
    includes: ['Global and project command scopes', 'Command editing and ordering', 'Command arguments']
  },
  {
    id: 'browser',
    group: 'workflows',
    title: 'Browser',
    description: 'Home page, link routing, and session cookies.',
    icon: Globe,
    keywords: ['browser', 'home page', 'search engine', 'site permissions', 'user agent'],
    implemented: false,
    includes: ['Home page and search engine', 'Site permissions', 'Default zoom and user agent']
  },
  {
    id: 'mobile-emulator',
    group: 'workflows',
    title: 'Mobile Emulator',
    description: 'Configure mobile emulator support for Orca and coding agents.',
    icon: TabletSmartphone,
    keywords: ['mobile emulator', 'android', 'ios', 'emulator'],
    implemented: false,
    includes: ['Emulator discovery', 'Device defaults', 'Agent emulator access']
  },
  {
    id: 'floating-workspace',
    group: 'workflows',
    title: 'Floating Workspace',
    description: 'Global terminal, browser, and markdown tabs.',
    icon: PanelsTopLeft,
    keywords: ['floating workspace', 'global tabs', 'terminal', 'markdown'],
    implemented: false,
    includes: ['Floating tab defaults', 'Global terminal access', 'Markdown tab behavior']
  },
  {
    id: 'appearance',
    group: 'interface',
    title: 'Appearance',
    description: 'Theme, zoom, app and terminal appearance, sidebars, and status bar.',
    icon: Palette,
    keywords: [
      'appearance',
      'theme',
      'dark',
      'light',
      'system',
      'language',
      'zoom',
      'scale',
      'font',
      'typography',
      'ide font',
      'cursor',
      'sidebar',
      'reduce visual effects',
      'terminal',
      'ui'
    ],
    implemented: true
  },
  {
    id: 'chat',
    group: 'interface',
    title: 'Chat UI',
    description: 'Choose how chats look and behave.',
    icon: MessageSquare,
    keywords: ['chat', 'native chat', 'agent permissions', 'naming', 'inline visuals'],
    implemented: false,
    includes: ['Agent launch preferences', 'Chat permissions', 'Chat naming and inline visuals']
  },
  {
    id: 'input',
    group: 'interface',
    title: 'Input & Editing',
    description: 'Selection and editing behavior.',
    icon: TextCursorInput,
    keywords: ['input', 'editing', 'selection', 'text field'],
    implemented: false,
    includes: ['Selection behavior', 'Editing commands', 'Text input preferences']
  },
  {
    id: 'notifications',
    group: 'interface',
    title: 'Notifications',
    description: 'Native desktop notifications for agent and terminal events.',
    icon: Bell,
    keywords: ['notifications', 'desktop alerts', 'sounds', 'terminal events'],
    implemented: true,
    includes: ['Agent and terminal notification sources', 'Notification sounds', 'Pending notification saves']
  },
  {
    id: 'shortcuts',
    group: 'interface',
    title: 'Shortcuts',
    description: 'Keyboard shortcuts for common actions.',
    icon: Keyboard,
    keywords: ['shortcuts', 'keyboard', 'keybindings', 'keys', 'bindings', 'chords'],
    implemented: true
  },
  {
    id: 'stats',
    group: 'interface',
    title: 'Stats & Usage',
    description:
      'Orca stats plus Claude, Codex, OpenCode, Muse token analytics and Grok subscription usage.',
    icon: BarChart3,
    keywords: ['stats', 'usage', 'tokens', 'analytics', 'grok', 'subscription'],
    implemented: true,
    includes: ['Token analytics', 'Subscription usage', 'Usage retention']
  },
  {
    id: 'ssh',
    group: 'remote',
    title: 'SSH Hosts',
    description: 'Use existing machines over SSH for files, terminals, Git, and workspaces.',
    icon: Cable,
    keywords: ['ssh', 'remote host', 'hosts', 'machines'],
    implemented: false,
    includes: ['Saved SSH hosts', 'Host authentication', 'Remote workspace setup']
  },
  {
    id: 'servers',
    group: 'remote',
    title: 'Remote Orca Servers',
    description: 'Run Orca on another machine and work on it from here.',
    icon: Server,
    keywords: ['remote server', 'runtime environment', 'orca server', 'host'],
    implemented: false,
    includes: ['Saved runtime servers', 'Server connection status', 'Managed server updates']
  },
  {
    id: 'privacy',
    group: 'security',
    title: 'Privacy & Telemetry',
    description: 'Anonymous usage data and telemetry controls.',
    icon: Lock,
    keywords: ['privacy', 'telemetry', 'usage data', 'diagnostics'],
    implemented: false,
    includes: ['Telemetry preference', 'Diagnostic bundle controls', 'Diagnostics history']
  },
  {
    id: 'advanced',
    group: 'advanced',
    title: 'Advanced',
    description: 'Low-level compatibility settings for troubleshooting.',
    icon: Wrench,
    keywords: ['advanced', 'compatibility', 'troubleshooting', 'developer tools'],
    implemented: false,
    includes: ['Compatibility flags', 'Experimental worktree card styles', 'Developer tooling']
  },
  {
    id: 'experimental',
    group: 'experimental',
    title: 'Experimental',
    description: 'New features that are still taking shape. Give them a try.',
    icon: FlaskConical,
    keywords: ['experimental', 'preview features', 'beta'],
    implemented: false,
    includes: ['Pre-release feature flags', 'Hidden experimental groups', 'Preview UI states']
  },
  {
    id: 'plugins',
    group: 'experimental',
    title: 'Plugins',
    description: 'Install and manage Orca plugins (Alpha).',
    icon: Blocks,
    badge: 'Alpha',
    keywords: ['plugins', 'extensions', 'marketplace', 'install'],
    implemented: false,
    includes: ['Plugin install and trust', 'Plugin updates', 'Plugin permissions']
  }
];

export function sectionById(id: string): NavSection | undefined {
  return NAV_SECTIONS.find((section) => section.id === id);
}
