<script lang="ts">
  // Orca components/settings/notifications/NotificationsPane.tsx (MIT), for local agent events.
  import { activity, playNotificationSound, setNotificationPrefs, showSystemNotification, type NotificationPrefs, type NotificationSound } from '../activity.svelte';
  import { errorMessage } from '../../api';
  import SettingRow from './SettingRow.svelte';
  import SettingsSwitch from './SettingsSwitch.svelte';

  let { query: _query = "" }: { query?: string } = $props();

  let testResult = $state<string | null>(null);
  const prefs = $derived(activity.prefs);

  function update(patch: Partial<NotificationPrefs>): void {
    setNotificationPrefs({ ...activity.prefs, ...patch });
  }

  async function sendTest(): Promise<void> {
    testResult = null;
    playNotificationSound(prefs.sound, prefs.volume);
    try {
      await showSystemNotification('Minnow', 'Notifications are working.');
      testResult = 'Sent. If nothing appeared, enable notifications for Minnow in Windows Settings.';
    } catch (error) {
      testResult = `Windows did not show the notification: ${errorMessage(error)}`;
    }
  }

  const SOUNDS: { id: NotificationSound; label: string }[] = [
    { id: 'none', label: 'None' },
    { id: 'two-tone', label: 'Two-tone' },
    { id: 'ding', label: 'Ding' },
    { id: 'blip', label: 'Blip' }
  ];
</script>

<div class="divide-y divide-border/60">
  <SettingRow label="Desktop Notifications" description="Show a Windows notification when an agent needs you.">
    <SettingsSwitch checked={prefs.enabled} ariaLabel="Desktop Notifications" onchange={() => update({ enabled: !prefs.enabled })} />
  </SettingRow>
  <SettingRow label="Agent Needs Input" description="Permission prompts and questions from an agent.">
    <SettingsSwitch checked={prefs.agentNeedsInput} disabled={!prefs.enabled} ariaLabel="Agent Needs Input" onchange={() => update({ agentNeedsInput: !prefs.agentNeedsInput })} />
  </SettingRow>
  <SettingRow label="Agent Task Complete" description="An agent went quiet after working.">
    <SettingsSwitch checked={prefs.agentTaskComplete} disabled={!prefs.enabled} ariaLabel="Agent Task Complete" onchange={() => update({ agentTaskComplete: !prefs.agentTaskComplete })} />
  </SettingRow>
  <SettingRow label="Terminal Exited" description="A terminal process ended on its own (not when you stop it).">
    <SettingsSwitch checked={prefs.terminalExited} disabled={!prefs.enabled} ariaLabel="Terminal Exited" onchange={() => update({ terminalExited: !prefs.terminalExited })} />
  </SettingRow>
  <SettingRow label="Suppress While Focused" description="Skip the notification while Minnow is the active window. Activity still records it.">
    <SettingsSwitch checked={prefs.suppressWhenFocused} disabled={!prefs.enabled} ariaLabel="Suppress While Focused" onchange={() => update({ suppressWhenFocused: !prefs.suppressWhenFocused })} />
  </SettingRow>
  <SettingRow label="Sound" description="Played with each notification.">
    <div class="flex items-center gap-2">
      <select
        aria-label="Notification sound"
        class="h-8 rounded-md border border-input bg-background px-2 text-sm"
        value={prefs.sound}
        onchange={(event) => { update({ sound: event.currentTarget.value as NotificationSound }); playNotificationSound(event.currentTarget.value as NotificationSound, prefs.volume); }}
      >
        {#each SOUNDS as sound (sound.id)}<option value={sound.id}>{sound.label}</option>{/each}
      </select>
      <input
        type="range"
        min="0"
        max="1"
        step="0.05"
        aria-label="Notification volume"
        value={prefs.volume}
        disabled={prefs.sound === 'none'}
        onchange={(event) => { update({ volume: Number(event.currentTarget.value) }); playNotificationSound(prefs.sound, Number(event.currentTarget.value)); }}
      />
    </div>
  </SettingRow>
  <SettingRow label="Test Notification" description={testResult ?? 'Send a notification now to check Windows delivery.'}>
    <button type="button" class="mn-button" onclick={() => void sendTest()}>Send Test</button>
  </SettingRow>
</div>
