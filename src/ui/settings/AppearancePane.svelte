<script lang="ts">
  // Orca 1.4.214 components/settings/appearance/AppearancePane.tsx,
  // AppearanceInterfaceSection.tsx, AppearanceSection.tsx and terminal appearance sections (MIT).
  import {
    AppWindow,
    ChevronDown,
    ChevronsUpDown,
    CircleX,
    FileUp,
    Ghost,
    Minus,
    PanelLeft,
    Plus,
    RotateCcw,
    SquareTerminal,
    WandSparkles
  } from 'lucide-svelte';
  import { defaultPreferences, type UiPreferences } from '../preferences';
  import { matchesSearch, normalizeQuery } from './search';
  import SettingsAccordion from './SettingsAccordion.svelte';
  import AppearanceAdvancedDisclosure from './AppearanceAdvancedDisclosure.svelte';
  import SettingRow from './SettingRow.svelte';
  import SettingsSwitch from './SettingsSwitch.svelte';
  import Segmented from './Segmented.svelte';
  import NumberField from './NumberField.svelte';
  import FontSizeStepper from './FontSizeStepper.svelte';
  import SubsectionHeader from './SubsectionHeader.svelte';
  import FontPicker from './FontPicker.svelte';
  import { buildTerminalFontFamily, firstFontFamily } from './font-family';

  let {
    preferences,
    onPreferences,
    query = ''
  }: {
    preferences: UiPreferences;
    onPreferences: (preferences: UiPreferences) => void;
    query?: string;
  } = $props();

  type SectionKey = 'interface' | 'terminal' | 'window';
  const ALL_SECTIONS: SectionKey[] = ['interface', 'terminal', 'window'];

  // The preference remains an integer pixel size; the reference control is a zoom percentage.
  const ZOOM_BASE_PX = 14;
  const ZOOM_MIN_PX = 11;
  const ZOOM_MAX_PX = 20;

  const importButtonClass =
    'inline-flex h-8 shrink-0 cursor-not-allowed items-center justify-center gap-1.5 rounded-md border border-border bg-background px-3 text-sm font-medium whitespace-nowrap text-foreground opacity-50 shadow-xs [&_svg]:size-4 [&_svg]:shrink-0 dark:border-input dark:bg-input/30';
  const zoomButtonClass =
    'inline-flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-background text-foreground shadow-xs transition-all outline-none hover:border-muted-foreground/35 hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 dark:border-input dark:bg-input/30 dark:hover:bg-input/50';
  const resetButtonClass =
    'ml-1 inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-md border border-border bg-background px-3 text-sm font-medium whitespace-nowrap text-foreground shadow-xs transition-all outline-none hover:border-muted-foreground/35 hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 dark:border-input dark:bg-input/30 dark:hover:bg-input/50';

  let openSections = $state<Set<SectionKey>>(new Set(ALL_SECTIONS));
  let searching = $derived(normalizeQuery(query).length > 0);

  let interfaceForce = $derived(
    !searching ||
      matchesSearch(query, {
        title: 'Interface',
        description: 'Theme, zoom, typography, and visual effects.',
        keywords: [
          'appearance',
          'interface',
          'theme',
          'language',
          'zoom',
          'scale',
          'font',
          'typography',
          'ide',
          'ui'
        ]
      })
  );
  let terminalForce = $derived(
    !searching ||
      matchesSearch(query, {
        title: 'Terminal',
        description: 'Terminal typography, cursor, and pane titles.',
        keywords: ['appearance', 'terminal', 'font', 'cursor']
      })
  );
  let windowForce = $derived(
    !searching ||
      matchesSearch(query, {
        title: 'Window & Sidebar',
        description: 'Sidebar, status bar, and file explorer',
        keywords: ['appearance', 'window', 'sidebar', 'layout']
      })
  );

  let themeVisible = $derived(
    interfaceForce ||
      rowMatches('Theme', '', ['dark', 'light', 'system', 'appearance', 'colors'])
  );
  let languageVisible = $derived(
    interfaceForce ||
      rowMatches('Language', 'Choose the language used by the interface.', [
        'language',
        'system',
        'english',
        'locale',
        'i18n',
        'translation'
      ])
  );
  let zoomVisible = $derived(
    interfaceForce ||
      rowMatches('UI Zoom', 'Scale the entire application interface.', [
        'zoom',
        'scale',
        'shortcut',
        'ui'
      ])
  );
  let ideFontVisible = $derived(
    interfaceForce ||
      rowMatches('IDE Font', 'Choose the font used by the interface.', [
        'font',
        'typeface',
        'typography',
        'ide',
        'ui',
        'interface',
        'geist'
      ])
  );
  let reduceEffectsVisible = $derived(
    rowMatches(
      'Reduce Visual Effects',
      'Turn off blur behind dialogs and menus, and window translucency.',
      ['blur', 'transparency', 'performance', 'remote', 'gpu', 'reduce visual effects']
    )
  );
  let advancedVisible = $derived(!searching || reduceEffectsVisible);

  let terminalSizeVisible = $derived(
    terminalForce ||
      rowMatches('Font Size', 'Default terminal font size for new panes and live updates.', [
        'terminal',
        'typography',
        'text size'
      ])
  );
  let terminalFontVisible = $derived(
    terminalForce ||
      rowMatches('Font Family', 'Typeface used in terminal panes.', [
        'terminal',
        'font',
        'typeface',
        'typography'
      ])
  );
  let lineHeightVisible = $derived(
    rowMatches('Line Height', 'Vertical spacing between terminal lines.', [
      'terminal',
      'typography',
      'line height',
      'spacing'
    ])
  );
  let typographySectionVisible = $derived(
    terminalForce || terminalSizeVisible || terminalFontVisible || lineHeightVisible
  );
  let typographyAdvancedVisible = $derived(
    !searching || terminalSizeVisible || terminalFontVisible || lineHeightVisible
  );
  let themeSectionVisible = $derived(
    terminalForce ||
      rowMatches('Terminal Themes', 'Terminal color themes and imports.', [
        'terminal',
        'theme',
        'themes',
        'color',
        'warp',
        'yaml',
        'import'
      ])
  );
  let cursorVisible = $derived(
    terminalForce ||
      rowMatches('Terminal Cursor', 'Cursor shape and blinking.', [
        'terminal',
        'cursor',
        'blink',
        'bar',
        'block',
        'underline'
      ])
  );
  let panesVisible = $derived(
    terminalForce ||
      rowMatches('Terminal Panes', 'Pane presentation preferences.', [
        'terminal',
        'pane',
        'pane titles',
        'title',
        'header'
      ])
  );

  let leftSidebarVisible = $derived(
    windowForce ||
      rowMatches('Left Sidebar Width', 'Width of the project and workspace sidebar.', [
        'sidebar',
        'width',
        'layout',
        'left'
      ])
  );
  let rightSidebarVisible = $derived(
    windowForce ||
      rowMatches('Right Sidebar Width', 'Width of the changes and details sidebar.', [
        'sidebar',
        'width',
        'layout',
        'right'
      ])
  );

  let interfaceMatches = $derived(
    interfaceForce ||
      themeVisible ||
      languageVisible ||
      zoomVisible ||
      ideFontVisible ||
      advancedVisible
  );
  let terminalMatches = $derived(
    terminalForce ||
      terminalSizeVisible ||
      terminalFontVisible ||
      lineHeightVisible ||
      themeSectionVisible ||
      cursorVisible ||
      panesVisible
  );
  let windowMatches = $derived(windowForce || leftSidebarVisible || rightSidebarVisible);
  let anyMatch = $derived(interfaceMatches || terminalMatches || windowMatches);

  let themeSummary = $derived(
    preferences.theme === 'system' ? 'System' : preferences.theme === 'light' ? 'Light' : 'Dark'
  );
  let interfaceSummary = $derived(`${themeSummary} · Geist`);
  let terminalSummary = $derived(
    `${firstFontFamily(preferences.terminalFontFamily)} · ${preferences.terminalFontSize}px`
  );
  let zoomPercent = $derived(Math.round((preferences.fontSize / ZOOM_BASE_PX) * 100));

  function rowMatches(title: string, description: string, keywords: string[]): boolean {
    return matchesSearch(query, { title, description, keywords });
  }

  function isOpen(key: SectionKey): boolean {
    if (searching) {
      return key === 'interface'
        ? interfaceMatches
        : key === 'terminal'
          ? terminalMatches
          : windowMatches;
    }
    return openSections.has(key);
  }

  function toggle(key: SectionKey): void {
    if (searching) return;
    const next = new Set(openSections);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    openSections = next;
  }

  function update(patch: Partial<UiPreferences>): void {
    onPreferences({ ...preferences, ...patch });
  }

  function stepZoom(delta: number): void {
    const next = Math.min(ZOOM_MAX_PX, Math.max(ZOOM_MIN_PX, preferences.fontSize + delta));
    if (next !== preferences.fontSize) update({ fontSize: next });
  }

  function resetZoom(): void {
    if (preferences.fontSize !== ZOOM_BASE_PX) update({ fontSize: ZOOM_BASE_PX });
  }

  const themeOptions = [
    { value: 'system', label: 'System' },
    { value: 'dark', label: 'Dark' },
    { value: 'light', label: 'Light' }
  ] as const;

  const cursorOptions = [
    { value: 'bar', label: 'Bar' },
    { value: 'block', label: 'Block' },
    { value: 'underline', label: 'Underline' }
  ] as const;
</script>

{#snippet keycap(label: string)}
  <span
    class="inline-flex min-w-6 items-center justify-center rounded border border-border/80 bg-secondary/70 px-1.5 py-0.5 text-xs font-medium text-muted-foreground shadow-sm"
  >
    {label}
  </span>
{/snippet}

{#snippet zoomHint()}
  <span class="inline-flex flex-wrap items-center gap-1">
    {@render keycap('Ctrl')}<span class="mx-0.5">+</span>{@render keycap('=')}
    <span class="mx-1">/</span>
    {@render keycap('Ctrl')}<span class="mx-0.5">+</span>{@render keycap('-')}
    <span class="ml-0.5">when not in a terminal pane.</span>
  </span>
{/snippet}

<div class="space-y-2.5">
  {#if !searching || anyMatch}
    {#if interfaceMatches}
      <SettingsAccordion
        id="interface"
        title="Interface"
        summary={interfaceSummary}
        open={isOpen('interface')}
        disabled={searching}
        onToggle={() => toggle('interface')}
      >
        {#snippet icon()}<AppWindow aria-hidden="true" />{/snippet}

        <div class="divide-y divide-border/40">
          {#if themeVisible}
            <SettingRow label="Theme">
              <Segmented
                ariaLabel="Theme"
                value={preferences.theme}
                options={themeOptions}
                onchange={(value) => update({ theme: value as UiPreferences['theme'] })}
              />
            </SettingRow>
          {/if}

          {#if languageVisible}
            <SettingRow label="Language">
              <div
                class="flex h-8 min-w-[220px] items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 text-sm whitespace-nowrap text-foreground opacity-60 shadow-xs dark:bg-input/30"
                title="Language selection is unavailable in this build"
              >
                <span>System</span>
                <ChevronDown class="size-4 opacity-50" aria-hidden="true" />
              </div>
            </SettingRow>
          {/if}

          {#if zoomVisible}
            <SettingRow label="UI Zoom">
              {#snippet description()}{@render zoomHint()}{/snippet}
              <div class="flex items-center gap-2">
                <button
                  type="button"
                  aria-label="Zoom out"
                  onclick={() => stepZoom(-1)}
                  disabled={preferences.fontSize <= ZOOM_MIN_PX}
                  class={zoomButtonClass}
                >
                  <Minus class="size-3" aria-hidden="true" />
                </button>
                <span class="w-14 text-center text-sm tabular-nums text-foreground">
                  {zoomPercent}%
                </span>
                <button
                  type="button"
                  aria-label="Zoom in"
                  onclick={() => stepZoom(1)}
                  disabled={preferences.fontSize >= ZOOM_MAX_PX}
                  class={zoomButtonClass}
                >
                  <Plus class="size-3" aria-hidden="true" />
                </button>
                <button
                  type="button"
                  aria-label="Reset zoom"
                  onclick={resetZoom}
                  disabled={preferences.fontSize === ZOOM_BASE_PX}
                  class={resetButtonClass}
                >
                  <RotateCcw class="size-3" aria-hidden="true" />
                  Reset
                </button>
              </div>
            </SettingRow>
          {/if}

          {#if ideFontVisible}
            <SettingRow label="IDE Font">
              <div
                class="relative w-[280px] max-w-full"
                title="Font selection is unavailable in this build"
              >
                <div
                  class="flex h-9 items-center rounded-md border border-input bg-transparent px-3 pr-16 text-sm text-foreground opacity-60 shadow-xs dark:bg-input/30"
                >
                  Geist
                </div>
                <div
                  class="pointer-events-none absolute inset-y-0 right-2 flex items-center gap-1 text-muted-foreground"
                >
                  <CircleX class="size-3.5" aria-hidden="true" />
                  <ChevronsUpDown class="size-3.5" aria-hidden="true" />
                </div>
              </div>
            </SettingRow>
          {/if}

          {#if advancedVisible}
            <AppearanceAdvancedDisclosure forceOpen={searching} showTopBorder={false}>
              <SettingRow
                label="Reduce Visual Effects"
                description="Turn off blur behind dialogs and menus, and window translucency after a restart. Helps remote desktops and machines without a GPU."
              >
                <SettingsSwitch
                  checked={preferences.reduceMotion}
                  ariaLabel="Reduce Visual Effects"
                  onchange={() => update({ reduceMotion: !preferences.reduceMotion })}
                />
              </SettingRow>
            </AppearanceAdvancedDisclosure>
          {/if}
        </div>
      </SettingsAccordion>
    {/if}

    {#if terminalMatches}
      <SettingsAccordion
        id="terminal"
        title="Terminal"
        summary={terminalSummary}
        open={isOpen('terminal')}
        disabled={searching}
        onToggle={() => toggle('terminal')}
      >
        {#snippet icon()}<SquareTerminal aria-hidden="true" />{/snippet}

        <div class="space-y-5">
          {#if typographySectionVisible}
          <section class="space-y-3 pt-2">
            <SubsectionHeader align="center" title="Terminal Typography">
              <span title="Import from Ghostty is unavailable in this build">
                <button
                  type="button"
                  disabled
                  aria-label="Import from Ghostty (unavailable in this build)"
                  class={importButtonClass}
                >
                  <Ghost aria-hidden="true" />
                  Import from Ghostty
                </button>
              </span>
            </SubsectionHeader>
            <div class="ml-4 divide-y divide-border/40 border-y border-border/40">
              {#if terminalSizeVisible}
                <SettingRow label="Font Size">
                  <FontSizeStepper
                    label="Font Size"
                    value={preferences.terminalFontSize}
                    min={8}
                    max={32}
                    onchange={(value) => update({ terminalFontSize: value })}
                  />
                </SettingRow>
              {/if}

              {#if terminalFontVisible}
                <SettingRow label="Font Family">
                  <div class="w-[280px] max-w-full">
                    <FontPicker
                      value={firstFontFamily(preferences.terminalFontFamily)}
                      ariaLabel="Font Family"
                      placeholder="Cascadia Code"
                      defaultValue={firstFontFamily(defaultPreferences.terminalFontFamily)}
                      suggestions={['Cascadia Mono', 'Consolas', 'Lucida Console']}
                      onchange={(value) =>
                        update({ terminalFontFamily: buildTerminalFontFamily(value) })}
                    />
                  </div>
                </SettingRow>
              {/if}
            </div>

            {#if typographyAdvancedVisible}
              <div class="ml-4">
                <AppearanceAdvancedDisclosure
                  forceOpen={searching}
                  showTopBorder={false}
                  contentClassName="ml-4"
                >
                  <SettingRow
                    label="Line Height"
                    description="Vertical spacing between terminal lines."
                  >
                    <NumberField
                      label="Line Height"
                      value={preferences.terminalLineHeight}
                      min={1}
                      max={2}
                      step={0.05}
                      suffix="×"
                      onchange={(value) => update({ terminalLineHeight: value })}
                    />
                  </SettingRow>
                </AppearanceAdvancedDisclosure>
              </div>
            {/if}
          </section>
          {/if}

          {#if themeSectionVisible}
            <section class="space-y-3">
              <SubsectionHeader align="center" title="Terminal Themes">
                <div class="flex flex-wrap items-center justify-end gap-2">
                  <span title="Import from Warp is unavailable in this build">
                    <button
                      type="button"
                      disabled
                      aria-label="Import from Warp (unavailable in this build)"
                      class={importButtonClass}
                    >
                      <WandSparkles aria-hidden="true" />
                      Import from Warp
                    </button>
                  </span>
                  <span title="Import from YAML is unavailable in this build">
                    <button
                      type="button"
                      disabled
                      aria-label="Import from YAML (unavailable in this build)"
                      class={importButtonClass}
                    >
                      <FileUp aria-hidden="true" />
                      Import from YAML
                    </button>
                  </span>
                </div>
              </SubsectionHeader>
              <div
                class="ml-4 rounded-lg border border-dashed border-border/60 px-4 py-4 text-xs text-muted-foreground"
              >
                Terminal color themes and imports are unavailable in this build. Terminal panes use
                the built-in theme.
              </div>
            </section>
          {/if}

          {#if cursorVisible}
            <section class="space-y-3">
              <SubsectionHeader title="Terminal Cursor" />
              <div class="ml-4 divide-y divide-border/40">
                <SettingRow
                  label="Cursor Shape"
                  description="Default cursor appearance for terminal panes."
                >
                  <Segmented
                    ariaLabel="Cursor Shape"
                    value={preferences.cursorStyle}
                    options={cursorOptions}
                    onchange={(value) =>
                      update({ cursorStyle: value as UiPreferences['cursorStyle'] })}
                  />
                </SettingRow>
                <SettingRow
                  label="Blinking Cursor"
                  description="Uses the blinking variant of the selected cursor shape."
                >
                  <SettingsSwitch
                    checked={preferences.cursorBlink}
                    ariaLabel="Blinking Cursor"
                    onchange={() => update({ cursorBlink: !preferences.cursorBlink })}
                  />
                </SettingRow>
              </div>
            </section>
          {/if}

          {#if panesVisible}
            <section class="space-y-3">
              <SubsectionHeader title="Terminal Panes" />
              <div class="ml-4 divide-y divide-border/40">
                <SettingRow
                  label="Show Pane Titles"
                  description="Display a title bar above each terminal pane."
                >
                  <SettingsSwitch
                    checked={preferences.paneTitles}
                    ariaLabel="Show Pane Titles"
                    onchange={() => update({ paneTitles: !preferences.paneTitles })}
                  />
                </SettingRow>
              </div>
            </section>
          {/if}
        </div>
      </SettingsAccordion>
    {/if}

    {#if windowMatches}
      <SettingsAccordion
        id="window"
        title="Window & Sidebar"
        summary="Sidebar, status bar, and file explorer"
        open={isOpen('window')}
        disabled={searching}
        onToggle={() => toggle('window')}
      >
        {#snippet icon()}<PanelLeft aria-hidden="true" />{/snippet}

        <div class="divide-y divide-border/40">
          {#if leftSidebarVisible}
            <SettingRow
              label="Left Sidebar Width"
              description="Width of the project and workspace sidebar."
            >
              <NumberField
                label="Left Sidebar Width"
                value={preferences.sidebarWidth}
                min={220}
                max={500}
                suffix="px"
                onchange={(value) => update({ sidebarWidth: value })}
              />
            </SettingRow>
          {/if}

          {#if rightSidebarVisible}
            <SettingRow
              label="Right Sidebar Width"
              description="Width of the changes and details sidebar."
            >
              <NumberField
                label="Right Sidebar Width"
                value={preferences.rightSidebarWidth}
                min={200}
                max={600}
                suffix="px"
                onchange={(value) => update({ rightSidebarWidth: value })}
              />
            </SettingRow>
          {/if}
        </div>
      </SettingsAccordion>
    {/if}
  {:else}
    <div
      class="rounded-md border border-dashed border-border/70 px-4 py-8 text-center text-sm text-muted-foreground"
    >
      No appearance settings match those filters.
    </div>
  {/if}
</div>
