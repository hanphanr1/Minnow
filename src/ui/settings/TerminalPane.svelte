<script lang="ts">
  // Orca 1.4.214 components/settings/terminal/TerminalPane.tsx plus
  // platform/TerminalWindows{Shell,Administrator}Section, TerminalRenderingSection,
  // TerminalContrastSetting and TerminalInteractionSection (MIT).
  // Unsupported source controls keep their footprints but render disabled with explicit hints.
  import { RotateCcw } from 'lucide-svelte';
  import { defaultPreferences, type UiPreferences } from '../preferences';
  import { matchesSearch, normalizeQuery } from './search';
  import SettingRow from './SettingRow.svelte';
  import SettingsSwitch from './SettingsSwitch.svelte';
  import SubsectionHeader from './SubsectionHeader.svelte';
  import Segmented from './Segmented.svelte';
  import NumberField from './NumberField.svelte';
  import FontSizeStepper from './FontSizeStepper.svelte';
  import FontPicker from './FontPicker.svelte';
  import UnavailableControl from './UnavailableControl.svelte';
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

  const outlineButtonClass =
    'inline-flex h-8 shrink-0 items-center justify-center gap-2 rounded-md border border-border bg-background px-3 text-sm font-medium whitespace-nowrap text-foreground shadow-xs dark:border-input dark:bg-input/30';
  const badgeClass =
    'inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-full border border-border px-2 py-0.5 text-xs font-medium whitespace-nowrap';

  const shellOptions = [
    { value: 'powershell.exe', label: 'PowerShell', disabled: true },
    { value: 'cmd.exe', label: 'Command Prompt', disabled: true },
    { value: 'git-bash', label: 'Git Bash', disabled: true }
  ] as const;

  const gpuOptions = [
    { value: 'auto', label: 'Auto', disabled: true },
    { value: 'on', label: 'On', disabled: true },
    { value: 'off', label: 'Off', disabled: true }
  ] as const;

  const contrastOptions = [
    { value: 'auto', label: 'Automatic', disabled: true },
    { value: 'off', label: 'Off', disabled: true },
    { value: 'custom', label: 'Custom', disabled: true }
  ] as const;

  const cursorOptions = [
    { value: 'bar', label: 'Bar' },
    { value: 'block', label: 'Block' },
    { value: 'underline', label: 'Underline' }
  ] as const;

  const scrollSliders = [
    {
      label: 'Normal',
      description: 'Scrollback wheel multiplier.',
      min: 0.5,
      max: 3,
      step: 0.05
    },
    {
      label: 'Fast',
      description: 'Extra multiplier while scrolling with a modifier key.',
      min: 1,
      max: 10,
      step: 0.5
    },
    {
      label: 'TUI',
      description: 'Discrete wheel reports for full-screen terminal apps.',
      min: 1,
      max: 10,
      step: 1
    }
  ] as const;

  let searching = $derived(normalizeQuery(query).length > 0);
  let pageForce = $derived(
    !searching ||
      matchesSearch(query, {
        title: 'Terminal',
        description: 'Shells, renderer, sessions, and terminal behavior.',
        keywords: ['terminal']
      })
  );

  function sectionMatches(title: string, description: string, keywords: string[]): boolean {
    return pageForce || matchesSearch(query, { title, description, keywords });
  }

  let adminVisible = $derived(
    sectionMatches(
      'Administrator mode',
      'Permissions for the Minnow app. Existing terminals may differ.',
      ['administrator', 'admin', 'elevation', 'permissions', 'standard user', 'restart']
    )
  );
  let shellVisible = $derived(
    sectionMatches('Windows Shell', 'Default shell for new terminal panes on Windows.', [
      'windows',
      'shell',
      'powershell',
      'cmd',
      'command prompt',
      'git bash',
      'default shell'
    ])
  );
  let renderingVisible = $derived(
    sectionMatches('Rendering', 'Terminal renderer behavior for live panes and new panes.', [
      'renderer',
      'rendering',
      'gpu',
      'webgl',
      'acceleration',
      'contrast',
      'inline images',
      'sixel',
      'transport'
    ])
  );
  let interactionVisible = $derived(
    sectionMatches('Terminal Interaction', 'Mouse and clipboard behavior for terminal panes.', [
      'scroll',
      'wheel',
      'scrollback',
      'clipboard',
      'copy',
      'paste',
      'focus',
      'mouse',
      'osc 52',
      'osc52'
    ])
  );
  let manageVisible = $derived(
    sectionMatches('Manage Sessions', 'Inspect terminal sessions running in this window.', [
      'sessions',
      'manage',
      'pty',
      'kill',
      'terminal sessions'
    ])
  );
  let appearanceVisible = $derived(
    sectionMatches(
      'Terminal Appearance',
      'Typography, cursor, and pane behavior for Minnow terminal panes.',
      ['font', 'typography', 'cursor', 'blink', 'pane titles', 'line height', 'appearance']
    )
  );
  let backendVisible = $derived(
    sectionMatches('Backend Limits', 'Fixed by the Minnow Rust backend.', [
      'backend',
      'scrollback',
      '2 mib',
      '64 kib',
      'buffer',
      'ack',
      'pty',
      'worktree',
      'limits'
    ])
  );

  let firstVisible = $derived(
    adminVisible
      ? 'admin'
      : shellVisible
        ? 'shell'
        : renderingVisible
          ? 'rendering'
          : interactionVisible
            ? 'interaction'
            : manageVisible
              ? 'manage'
              : appearanceVisible
                ? 'appearance'
                : backendVisible
                  ? 'backend'
                  : ''
  );
  let anyVisible = $derived(firstVisible !== '');

  function update(patch: Partial<UiPreferences>): void {
    onPreferences({ ...preferences, ...patch });
  }
</script>

{#snippet fixedBadge()}
  <span
    class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium tracking-[0.05em] text-muted-foreground uppercase"
  >
    Fixed
  </span>
{/snippet}

<div class="space-y-6">
  {#if !searching || anyVisible}
    {#if adminVisible}
      <section class="space-y-3">
        <SettingRow description="Permissions for the Minnow app. Existing terminals may differ.">
          {#snippet label()}
            <span class="flex flex-wrap items-center gap-2">
              Administrator mode
              <span role="status" class={badgeClass}>Standard user</span>
            </span>
          {/snippet}
          <UnavailableControl title="Restart as administrator is unavailable in this build">
            <button
              type="button"
              disabled
              aria-label="Restart as administrator (unavailable in this build)"
              class={outlineButtonClass}
            >
              Restart as administrator
            </button>
          </UnavailableControl>
        </SettingRow>
      </section>
    {/if}

    {#if shellVisible}
      {#if firstVisible !== 'shell'}<div class="h-px bg-border/40"></div>{/if}
      <section class="space-y-3">
        <SubsectionHeader
          title="Windows Shell"
          description="Default shell for new terminal panes on Windows."
        />
        <div class="divide-y divide-border/40">
          <SettingRow
            label="Default Shell"
            description="Shell used when opening a new terminal pane. Takes effect for new terminals."
          >
            <UnavailableControl
              title="Minnow starts the OMP executable, so Windows shell selection is unavailable"
              hint="Starts OMP"
            >
              <div class="w-[330px] max-w-full">
                <Segmented
                  ariaLabel="Default Shell"
                  value="powershell.exe"
                  size="sm"
                  equalWidth
                  options={shellOptions}
                  onchange={() => {}}
                />
              </div>
            </UnavailableControl>
          </SettingRow>
        </div>
      </section>
    {/if}

    {#if renderingVisible}
      {#if firstVisible !== 'rendering'}<div class="h-px bg-border/40"></div>{/if}
      <section class="space-y-3">
        <SubsectionHeader
          title="Rendering"
          description="Terminal renderer behavior for live panes and new panes."
        />
        <div class="divide-y divide-border/40">
          <SettingRow
            label="GPU Acceleration"
            description="Auto tries WebGL, with DOM fallback for unsupported or risky renderers."
          >
            <UnavailableControl title="GPU acceleration is fixed in this build">
              <Segmented
                ariaLabel="GPU Acceleration"
                value="auto"
                options={gpuOptions}
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
          <SettingRow
            label="Color Contrast"
            description="Balances readability with your terminal theme. Recommended."
          >
            <UnavailableControl title="Terminal contrast is fixed in this build">
              <Segmented
                ariaLabel="Color Contrast"
                value="auto"
                options={contrastOptions}
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
          <SettingRow
            label="Inline Images"
            description="Display images directly in the terminal using SIXEL, iTerm2 (IIP), and Kitty graphics protocols."
          >
            <UnavailableControl title="Inline terminal images are unavailable in this build">
              <SettingsSwitch
                checked={false}
                disabled
                ariaLabel="Inline Images"
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
        </div>
      </section>
    {/if}

    {#if interactionVisible}
      {#if firstVisible !== 'interaction'}<div class="h-px bg-border/40"></div>{/if}
      <section class="space-y-3">
        <SubsectionHeader
          title="Terminal Interaction"
          description="Mouse and clipboard behavior for terminal panes."
        />
        <div class="divide-y divide-border/40">
          <div class="space-y-3 py-3">
            <div class="flex flex-wrap items-start justify-between gap-3">
              <div class="space-y-0.5">
                <div class="text-sm leading-none font-medium">Scroll Speed</div>
                <p class="max-w-xl text-xs text-muted-foreground">
                  Adjust how wheel input feels in scrollback and in mouse-aware terminal apps.
                </p>
              </div>
              <UnavailableControl title="Terminal scroll speed is unavailable in this build">
                <button
                  type="button"
                  disabled
                  aria-label="Reset scroll speed (unavailable in this build)"
                  class={outlineButtonClass}
                >
                  <RotateCcw class="size-3.5" aria-hidden="true" />
                  Reset
                </button>
              </UnavailableControl>
            </div>
            <div class="grid gap-3 md:grid-cols-3">
              {#each scrollSliders as slider (slider.label)}
                <div class="rounded-md border border-border/60 bg-background/50 p-3 opacity-60">
                  <div class="flex items-start justify-between gap-3">
                    <div class="min-w-0 space-y-0.5">
                      <div class="text-xs font-medium">{slider.label}</div>
                      <p class="text-[11px] leading-4 text-muted-foreground">
                        {slider.description}
                      </p>
                    </div>
                    <span
                      class="shrink-0 rounded-md border border-border/50 bg-muted/40 px-1.5 py-0.5 font-mono text-[11px] tabular-nums text-foreground"
                    >
                      —
                    </span>
                  </div>
                  <input
                    type="range"
                    disabled
                    min={slider.min}
                    max={slider.max}
                    step={slider.step}
                    value={slider.min}
                    aria-label={`${slider.label} scroll speed (unavailable in this build)`}
                    class="mt-3 w-full accent-foreground"
                  />
                  <div class="mt-1 flex justify-between font-mono text-[10px] text-muted-foreground">
                    <span>{slider.min}</span>
                    <span>{slider.max}</span>
                  </div>
                </div>
              {/each}
            </div>
          </div>

          <SettingRow
            label="Scroll to Bottom on Input"
            description="Typing or pasting while scrolled up jumps to the latest output. Turn off to keep your place."
          >
            <UnavailableControl title="Input scrolling behavior is unavailable in this build">
              <SettingsSwitch
                checked={false}
                disabled
                ariaLabel="Scroll to Bottom on Input"
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
          <SettingRow
            label="Right-click to paste"
            description="Right-click pastes the clipboard. Ctrl+right-click opens the context menu."
          >
            <UnavailableControl title="Right-click paste is unavailable in this build">
              <SettingsSwitch
                checked={false}
                disabled
                ariaLabel="Right-click to paste"
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
          <SettingRow
            label="Focus Follows Mouse"
            description="Hovering a terminal pane activates it without needing to click."
          >
            <UnavailableControl title="Focus follows mouse is unavailable in this build">
              <SettingsSwitch
                checked={false}
                disabled
                ariaLabel="Focus Follows Mouse"
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
          <SettingRow
            label="Copy on Select"
            description="Automatically copy text you select in a terminal or chat to the clipboard."
          >
            <UnavailableControl title="Copy on select is unavailable in this build">
              <SettingsSwitch
                checked={false}
                disabled
                ariaLabel="Copy on Select"
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
          <SettingRow
            label="Trim Gutter on Copy"
            description="Drop the left gutter agent output is painted behind, so copied text is not indented. Only the indent every selected line shares is removed."
          >
            <UnavailableControl title="Gutter trimming on copy is unavailable in this build">
              <SettingsSwitch
                checked={false}
                disabled
                ariaLabel="Trim Gutter on Copy"
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
          <SettingRow
            label="Allow TUI Clipboard Writes (OSC 52)"
            description="Let programs in the terminal (Zellij, tmux, Neovim, fzf, Grok, SSH) copy to your system clipboard."
          >
            <UnavailableControl title="OSC 52 clipboard writes are unavailable in this build">
              <SettingsSwitch
                checked={false}
                disabled
                ariaLabel="Allow TUI Clipboard Writes (OSC 52)"
                onchange={() => {}}
              />
            </UnavailableControl>
          </SettingRow>
        </div>
      </section>
    {/if}

    {#if manageVisible}
      {#if firstVisible !== 'manage'}<div class="h-px bg-border/40"></div>{/if}
      <section class="space-y-3">
        <SubsectionHeader
          title="Manage Sessions"
          description="Inspect terminal sessions running in this window."
        />
        <div
          class="rounded-lg border border-dashed border-border/60 px-4 py-4 text-xs text-muted-foreground"
        >
          Session management is unavailable in this build. Manage workers from the workspace list.
        </div>
      </section>
    {/if}

    {#if appearanceVisible}
      {#if firstVisible !== 'appearance'}<div class="h-px bg-border/40"></div>{/if}
      <section class="space-y-4">
        <SubsectionHeader
          title="Terminal Appearance"
          description="Typography, cursor, and pane behavior for Minnow terminal panes."
        />
        <div class="space-y-4">
          <div class="space-y-3">
            <SubsectionHeader title="Terminal Typography" />
            <div class="ml-4 divide-y divide-border/40 border-y border-border/40">
              <SettingRow label="Font Size">
                <FontSizeStepper
                  label="Font Size"
                  value={preferences.terminalFontSize}
                  min={8}
                  max={32}
                  onchange={(value) => update({ terminalFontSize: value })}
                />
              </SettingRow>
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
              <SettingRow label="Line Height">
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
            </div>
          </div>

          <div class="space-y-3">
            <SubsectionHeader title="Terminal Cursor" />
            <div class="ml-4 divide-y divide-border/40">
              <SettingRow label="Cursor Shape">
                <Segmented
                  ariaLabel="Cursor Shape"
                  value={preferences.cursorStyle}
                  options={cursorOptions}
                  onchange={(value) =>
                    update({ cursorStyle: value as UiPreferences['cursorStyle'] })}
                />
              </SettingRow>
              <SettingRow label="Blinking Cursor">
                <SettingsSwitch
                  checked={preferences.cursorBlink}
                  ariaLabel="Blinking Cursor"
                  onchange={() => update({ cursorBlink: !preferences.cursorBlink })}
                />
              </SettingRow>
            </div>
          </div>

          <div class="space-y-3">
            <SubsectionHeader title="Terminal Panes" />
            <div class="ml-4 divide-y divide-border/40">
              <SettingRow label="Show Pane Titles">
                <SettingsSwitch
                  checked={preferences.paneTitles}
                  ariaLabel="Show Pane Titles"
                  onchange={() => update({ paneTitles: !preferences.paneTitles })}
                />
              </SettingRow>
            </div>
          </div>
        </div>
      </section>
    {/if}

    {#if backendVisible}
      {#if firstVisible !== 'backend'}<div class="h-px bg-border/40"></div>{/if}
      <section class="space-y-3">
        <SubsectionHeader
          title="Backend Limits"
          description="Fixed by the Minnow Rust backend; not configurable from Settings."
        />
        <div class="divide-y divide-border/40">
          <SettingRow
            label="Scrollback Buffer"
            description="Each worker keeps the most recent 2 MiB of terminal output in memory."
          >
            {@render fixedBadge()}
          </SettingRow>
          <SettingRow
            label="Write Acknowledgement"
            description="Terminal chunks are acknowledged in 64 KiB windows for backpressure."
          >
            {@render fixedBadge()}
          </SettingRow>
          <SettingRow
            label="Session Model"
            description="One worker owns one Git worktree and one PTY. Opening another terminal launches another worker."
          >
            {@render fixedBadge()}
          </SettingRow>
          <SettingRow
            label="OMP Executable"
            description="Workers start the omp executable configured on the General page."
          >
            {@render fixedBadge()}
          </SettingRow>
        </div>
      </section>
    {/if}
  {:else}
    <div
      class="rounded-md border border-dashed border-border/70 px-4 py-8 text-center text-sm text-muted-foreground"
    >
      No terminal settings match those filters.
    </div>
  {/if}
</div>
