# Orca reference study

Source: [stablyai/orca](https://github.com/stablyai/orca), cloned to `_ref/orca`, pinned at `e507dd7732139e302f6adaa2917d8e0aee5b1362`. This is source inspection, not an Orca RAM benchmark. Constants below describe that revision, not measured memory consumption.

## License and reuse

[Orca's LICENSE](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/LICENSE) is MIT, copyright 2026 Lovecast Inc. The notice must accompany copied substantial portions. omp-deck independently implements the requested interaction model in Rust/Svelte; it ports no Orca source or assets. The full reference notice is retained in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Worktrees and workers

Orca separates a Git worktree, its terminal resources, and orchestration worker records. Workers belong to a SQLite-backed dispatch graph rather than being only child-process handles. The launch catalogue supports many different agent CLIs and their readiness/paste conventions ([catalogue](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/shared/agent-launch/catalog/tui-agent-config.ts#L15-L355)).

Worktree creation is exposed through `worktrees:create` and related IPC handlers ([handlers](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/ipc/worktrees.ts#L35-L67)). A per-project in-flight guard and synchronous path claims prevent duplicate creation ([guard](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/workspaces/creation/worktree-create-in-flight.ts#L16-L83)). Deletion uses a separate two-operation concurrency limit ([limit](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/git/worktrees/removal/worktree-delete-limit.ts#L1-L12)); scans coalesce by repository and generation ([scan cache](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/git/worktrees/catalog/worktree-scan-cache.ts#L57-L88)).

omp-deck keeps the one-worker/one-worktree/one-PTY relationship, serializes lifecycle mutations, and delegates safety checks to explicit Git commands. It does not implement dispatch graphs, remote workers, nested-agent control, or Orca's multi-vendor launch catalogue.

## PTY ownership and transport

Orca's PTYs do **not** live directly in Electron's renderer or main process. A separate native PTY daemon owns them; Electron main communicates over a framed socket/pipe ([daemon launcher](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/daemon/launch/daemon-out-of-process-launcher.ts#L79-L260)). A daemon-side headless emulator is the screen authority ([output plane](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/daemon/sessions/session-output-plane.ts#L17-L72)). This supports renderer recreation and remote/headless clients, at the cost of another process and terminal model.

The live daemon scrollback defaults to 1,000 rows ([policy](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/daemon/sessions/daemon-session-scrollback-window.ts#L5-L8)). Pending output is capped at 2 MiB and adjacent records coalesce below 64 KiB. Socket batching has a 2 ms interval, 128 KiB shallow gate, 64 KiB bulk slices, and a separate 32 MiB fallback cap ([batcher](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/daemon/transport/stream/daemon-stream-data-batcher.ts#L35-L46)). Interactive output can bypass batching ([admission](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/daemon/server/daemon-terminal-admission.ts#L205-L214)).

omp-deck uses portable-pty/ConPTY directly in Rust, blocking readers, a bounded byte ring, and acknowledgement-limited Tauri channels. It intentionally has no daemon-side headless emulator or persistent PTY daemon. The tradeoff is simpler ownership and less duplicated terminal state, but replay is a bounded byte history rather than a fully reconstructed screen snapshot. App restart relaunches omp rather than adopting an existing process.

## Hidden terminals: existing Orca optimizations

It would be incorrect to say Orca renders every hidden terminal continuously:

- Main suppresses hidden-pane bytes when no sidecar needs them and requests model restoration on reveal ([delivery gate](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/ipc/pty/delivery/pty-hidden-delivery-gate.ts#L90-L99)).
- Renderer output queues are bounded and time-budgeted; overflow discards output and writes a visible warning ([queue policy](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/lib/pane-manager/pane-terminal-output-queue-registry.ts#L93-L118)).
- Restorable hidden views can cold-park after 30 seconds, with limited warm retention ([parking policy](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/components/terminal-pane/terminal-hidden-view-parking.ts#L9-L15)).
- Hidden WebGL contexts use an LRU cap of six ([retention](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/lib/pane-manager/terminal-webgl-hidden-retention.ts#L5)); hidden canvas backing stores can be released after 30 seconds ([canvas release](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/lib/pane-manager/terminal-webgl-hidden-canvas-release.ts#L2)).

The child agent processes still run, and daemon terminal models still occupy memory. omp-deck disposes hidden xterm/WebGL views rather than retaining a warm set, while its Rust rings continue receiving output.

## Agent status detection

Orca combines managed agent hooks, terminal OSC status frames, program activity records, and terminal-title heuristics. OSC 9999 carries agent status and OSC 7501 carries program activity; parsing carries partial frames across chunks and bounds pending input ([OSC parser](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/shared/agent-status/transport/agent-status-osc.ts#L9-L73)). The local hook server and one ingestion point reconcile competing authorities, deduplicate events, and apply precedence/yield windows ([merge point](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/agent-hooks/server/server-ingest-terminal.ts#L82-L319)).

The renderer status store caps both retained agents and live statuses at 500 ([caps](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/store/slices/agent-status/agent-status-capacity-eviction.ts#L9-L27)). Freshness decay uses deadlines rather than rapid polling ([scheduler](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/store/slices/agent-status/agent-status-freshness-scheduler.ts#L126-L160)).

omp-deck uses the explicitly requested PTY-activity/process-state approach. It does not install hooks in the user's omp configuration. Silence and prompt matching are heuristics and do not prove that a model turn completed; README documents that distinction.

## Notifications

Orca's delivery service applies read/mute/status/focus gates, mobile fan-out, a burst cooldown, and OS delivery ([service](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/notifications/notification-delivery-service.ts#L60-L224)). The cooldown is five seconds with at most 50 recent dedupe keys ([cooldown](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/shared/notification-burst-cooldown.ts#L1-L2)). Clicking a notification routes focus to a stable terminal leaf ([native delivery](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/ipc/native-notification-delivery.ts#L114-L135)).

omp-deck sends native Windows notifications on idle and exit transitions. It does not reproduce mobile forwarding, tray attention, or pane-targeted notification clicks.

## Session persistence and resume

Orca's profile-state authority is SQLite, with a marker protecting against stale legacy JSON resurrection ([profile state](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/persistence/profile-state/profile-state-access.ts#L37-L38)). Writes debounce for one second with a five-second maximum wait ([write scheduling](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/persistence/loading-store/write-scheduling.ts#L17-L18)). Atomic snapshot writes use generation fences ([writer](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/persistence/atomic-snapshot-writer.ts#L100-L160)). Renderer persistence checkpoints on shutdown and periodically captures resume IDs ([session persistence](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/app-shell/use-app-session-persistence.ts#L37-L115)).

omp-deck stores project/worker identity, paths, merge bases, desired-running state, and settings in SQLite. omp owns conversation persistence inside a dedicated directory per worker. Restart recovery reuses that directory; it does not attempt Orca's remote/sleep/relay recovery matrix.

## Layout and interaction

Orca models workspaces, groups, tabs, and split-pane trees. The terminal workbench uses `display:none`, `inert`, and `aria-hidden` when not active ([container](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/components/TerminalWorkbenchContainer.tsx#L20-L26)). Its renderer combines many Zustand slices ([store assembly](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/store/index.ts#L110-L170)); lists virtualize above 50 rows with overscan 10 ([list](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/renderer/src/components/virtualized-list.tsx#L5-L7)).

omp-deck retains project navigation, named worker terminals, grid/tab switching, focused actions, and local dialogs. It omits general split-tree editing and unrelated workbench surfaces.

## Where weight comes from

| Source-level cost | What omp-deck changes |
| --- | --- |
| Electron ships Chromium and a Node runtime; browser, renderer, GPU, and utility processes have baseline cost. | Tauri uses Windows' installed WebView2 plus Rust. WebView2 processes and GPU memory still exist. |
| Electron main plus a separate PTY daemon; daemon terminal models plus renderer xterm models. | One Rust backend owns PTYs; byte rings replace daemon emulators. |
| Broad renderer state, many workbench surfaces, retained warm views, and integration state. | Small Svelte state model; dispose hidden terminal views. |
| Multi-vendor agent catalogues/hooks, usage scanners, orchestration mailboxes, telemetry, remote/WSL support. | Only local Git worktrees and omp CLI sessions. |
| Timed work and diagnostics, including provider scanning and Windows process-resource collection. | No live resource dashboard; one-second inactivity checks, blocking exit waits, explicit measurement script. |

Concrete timed examples in Orca include a five-second desktop-presence report ([presence](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/notifications/desktop-presence-reporter.ts#L7-L8)) and Windows CIM/typeperf resource collection with a five-second timeout, 10 MiB output cap, and 30-second CIM backoff ([collector](https://github.com/stablyai/orca/blob/e507dd7732139e302f6adaa2917d8e0aee5b1362/src/main/memory/windows-process-resource-collector.ts#L19-L105)). Those show complexity and possible overhead, not proof of a specific RAM total or continuous rapid polling.

No Orca memory number is inferred from its source. omp-deck's own release process-tree measurements belong in README and the accompanying measurement evidence.
