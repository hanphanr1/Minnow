<script lang="ts">
  // Orca 1.4.214 components/settings/Settings.tsx and
  // components/settings/page/settings-page-renderer.tsx (MIT, © 2026 Lovecast Inc.)
  // Full settings page: 280px navigation, source group order, section search and panes.
  // Mount contract: parent mounts this directly as a flex child above the global 24px status
  // bar with no titlebar. Settings self-renders the 36px drag gap and the 138px controls reserve.
  import { untrack } from 'svelte';
  import { ArrowLeft, Ellipsis } from 'lucide-svelte';
  import minnowMark from '../../src-tauri/icons/32x32.png';
  import SidebarMenu from './sidebar/SidebarMenu.svelte';
  import type { MenuAnchor } from './sidebar/presentation';
  import type { Settings as AppSettings } from '../types';
  import type { UiPreferences } from './preferences';
  import { NAV_GROUPS, NAV_SECTIONS, type NavSection } from './settings/settings-data';
  import { shortcutDefinitions } from './settings/shortcut-catalog';
  import { normalizeQuery, rankSections } from './settings/search';
  import SettingsSidebar from './settings/SettingsSidebar.svelte';
  import SettingsSection from './settings/SettingsSection.svelte';
  import AppearancePane from './settings/AppearancePane.svelte';
  import TerminalPane from './settings/TerminalPane.svelte';
  import GeneralPane from './settings/GeneralPane.svelte';
  import ShortcutsPane from './settings/ShortcutsPane.svelte';
  import UnavailablePane from './settings/UnavailablePane.svelte';
  import QuickCommandsPane from './settings/QuickCommandsPane.svelte';
  import NotificationsPane from './settings/NotificationsPane.svelte';
  import type { Project } from '../types';

  let {
    settings,
    preferences,
    onSave,
    onPreferences,
    onBack,
    initialSection,
    projects = []
  }: {
    settings: AppSettings;
    preferences: UiPreferences;
    onSave: (settings: AppSettings) => Promise<void>;
    onPreferences: (preferences: UiPreferences) => void;
    onBack: () => void;
    initialSection?: string;
    projects?: Project[];
  } = $props();

  // Deep links only choose the opening section; later navigation owns activeSectionId.
  const startSection = untrack(() =>
    initialSection && NAV_SECTIONS.some((section) => section.id === initialSection)
      ? initialSection
      : 'appearance'
  );

  let activeSectionId = $state(startSection);
  let query = $state('');
  let contentEl: HTMLDivElement | undefined = $state();
  let previousQuery = untrack(() => query);

  let appMenu = $state<MenuAnchor|null>(null);
  let menuOpener = $state<HTMLButtonElement>();
  // Orca indexes each shortcut row in settings search; fold the catalog into the section entry.
  const searchableSections = NAV_SECTIONS.map((section) =>
    section.id === 'shortcuts'
      ? {
          ...section,
          keywords: [
            ...section.keywords,
            ...shortcutDefinitions.flatMap((definition) => [
              definition.title,
              ...definition.keywords,
              ...definition.keys.flat()
            ])
          ]
        }
      : section
  );

  let searching = $derived(normalizeQuery(query).length > 0);
  let rankedSections = $derived(rankSections(query, searchableSections) as NavSection[]);
  let matchedIds = $derived(new Set(rankedSections.map((section) => section.id)));
  let visibleSections = $derived(
    searching
      ? // The reference ranks the navigation but renders matched panes in source order.
        NAV_SECTIONS.filter((section) => matchedIds.has(section.id))
      : NAV_SECTIONS.filter((section) => section.id === activeSectionId)
  );
  let focusedShortcuts = $derived(!searching && activeSectionId === 'shortcuts');

  let sidebarGroups = $derived.by(() => {
    const source = searching ? rankedSections : NAV_SECTIONS;
    return NAV_GROUPS.map((group) => ({
      group,
      // The onboarding row is pinned directly below search, like the reference sidebar.
      sections: source.filter(
        (section) => section.group === group.id && section.id !== 'setup-guide'
      )
    })).filter((entry) => entry.sections.length > 0);
  });

  $effect(() => {
    if (query !== previousQuery) {
      previousQuery = query;
      contentEl?.scrollTo({ top: 0 });
    }
  });

  function selectSection(id: string): void {
    if (query !== '') {
      // Clear stale filtering when selecting a result’s pane, like the reference client.
      query = '';
    }
    activeSectionId = id;
    contentEl?.scrollTo({ top: 0 });
  }

  function updateQuery(value: string): void {
    query = value;
  }
</script>

<div class="flex h-full min-h-0 flex-1 flex-col bg-background">
  <!-- The parent has no window titlebar: self-render the 36px drag gap and reserve the
       fixed 138×36 native controls that overlay this row. -->
  <div class="titlebar shrink-0" data-tauri-drag-region>
    <div class="flex h-full items-center gap-1 px-3">
      <img src={minnowMark} alt="" class="size-4" aria-hidden="true"/>
      <button bind:this={menuOpener} class="mn-icon-button" aria-label="Application menu" aria-haspopup="menu" aria-expanded={appMenu !== null} onclick={event => { const rect = event.currentTarget.getBoundingClientRect(); appMenu = appMenu ? null : {left:rect.left,top:rect.top,bottom:rect.bottom,right:rect.right}; }}><Ellipsis size={16}/></button>
    </div>
    <div class="h-full flex-1" data-tauri-drag-region aria-hidden="true"></div>
    <div class="window-controls-titlebar-spacer" aria-hidden="true"></div>
  </div>

  <div class="settings-view-shell flex min-h-0 flex-1 overflow-hidden bg-background">
    <div class="settings-navigation flex min-h-0 shrink-0">
    <SettingsSidebar
    {activeSectionId}
    {query}
    groups={sidebarGroups}
    autoFocus={!initialSection}
    onQuery={updateQuery}
    onSelect={selectSection}
    {onBack}
  />
    </div>

  <div class="settings-content flex min-h-0 min-w-0 flex-1 flex-col">
    <div
      bind:this={contentEl}
      class={`min-h-0 flex-1 ${
        focusedShortcuts
          ? 'overflow-hidden'
          : 'overflow-y-auto scrollbar-sleek [scrollbar-gutter:stable]'
      }`}
    >
      <div
        class={`settings-content-band mx-auto flex w-full flex-col gap-10 px-8 pt-10 ${
          focusedShortcuts ? 'h-full pb-6' : 'pb-24'
        } max-w-4xl`}
      >
        {#if visibleSections.length === 0}
          <div
            class="flex min-h-[24rem] items-center justify-center rounded-2xl border border-dashed border-border/60 bg-card/30 text-sm text-muted-foreground"
          >
            No settings found for "{query.trim()}"
          </div>
        {:else}
          {#each visibleSections as section (section.id)}
            <SettingsSection
              id={section.id}
              title={section.title}
              description={section.description}
              badge={section.badge}
              focused={focusedShortcuts && section.id === 'shortcuts'}
            >
              {#if section.id === 'appearance'}
                <AppearancePane {preferences} {onPreferences} {query} />
              {:else if section.id === 'terminal'}
                <TerminalPane {preferences} {onPreferences} {query} />
              {:else if section.id === 'general'}
                <GeneralPane {settings} {onSave} {query} />
              {:else if section.id === 'notifications'}
                <NotificationsPane {query} />
              {:else if section.id === 'quick-commands'}
                <QuickCommandsPane {projects} {query} />
              {:else if section.id === 'shortcuts'}
                <ShortcutsPane {query} />
              {:else}
                <UnavailablePane {section} />
              {/if}
            </SettingsSection>
          {/each}
        {/if}
      </div>
    </div>
  </div>
  </div>
</div>
{#if appMenu}
  <SidebarMenu anchor={appMenu} label="Application menu" restoreTo={menuOpener} items={[{id:'back',label:'Back to app',icon:ArrowLeft,onSelect:onBack}]} onClose={() => { appMenu = null; }}/>
{/if}

<style>
  .settings-content { container: settings-content / inline-size; }
  @media (max-width: 899px) {
    .settings-view-shell { flex-direction: column; }
    .settings-navigation { height: 240px; border-bottom: 1px solid var(--border); }
    .settings-navigation :global(aside) { width: 100%; }
    .settings-navigation :global(aside > div) { padding-top: 6px; padding-bottom: 6px; }
    .settings-content-band { padding-left: 16px; padding-right: 16px; padding-top: 24px; }
  }
</style>
