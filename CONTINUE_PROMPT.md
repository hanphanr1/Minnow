# Prompt bàn giao Minnow cho agent tiếp theo

Tiếp tục project **Minnow** tại `C:/Users/DELL/Documents/ORCA TAURI` (repo `https://github.com/hanphanr1/Minnow.git`, branch `main`). Minnow là app Windows Tauri 2 + Rust + Svelte 5, clone UX của Orca để quản lý omp CLI agents trong Git worktree. Ưu tiên nhẹ RAM, UI nhanh, không Electron. Mục tiêu của user: **parity Orca đầy đủ**. Không viết lại project; tiếp tục từ trạng thái hiện có.

Đọc theo thứ tự: `FEATURES.md` (bảng mọi tính năng Orca và trạng thái), `SESSION_HANDOFF.md`, `TERMINAL_CUTOVER.md`, `TODO.md`, `UI_PARITY.md`. Source Orca tham chiếu: `_ref/orca` (MIT, pin `e507dd7`). Port hành vi/chữ/kiểu dáng từ đó, giữ attribution.

## Đã có (commit local, CHƯA push; push chỉ khi user đồng ý)

- `857d723` nhiều terminal độc lập mỗi worktree (TerminalSession, SQLite migration, OSC 7 split cwd, close/reopen, cold resume).
- `e8c0470` Explorer Names/Contents, Find files native, Git decorations, dotfile/ignored toggles.
- `cca7df5` file/diff là tab trong workspace group, guard dirty.
- `adcc69a` Quick Open Ctrl+P.
- `476f0c0` file ops Explorer (new/rename/delete→trash/reveal, context menu).
- `60f462b` Source Control discard.
- `56bf275` Quick Commands, Notifications (UI quyết định, native chỉ hiển thị), trang Activity.
- `007dd9d` Agent Session Search + Stats & Usage từ transcript omp.
- `b9adf07` Ports panel (PID của job terminal + `netstat`), Source Control "Load more commits" (tới 500) và mục Merge Changes. **Đã qua check/build/47 native tests nhưng CHƯA smoke native** — làm việc đó đầu tiên.

## Việc tiếp theo (theo thứ tự)

1. Smoke native Ports (chạy `python -m http.server 8765` trong một shell terminal; panel Ports phải thấy port; nút mở browser) và Source Control (tạo conflict trong `.smoke/repo`, Merge Changes mở file; Load more).
2. Workspace cleanup (Orca `workspace-cleanup`): liệt kê worktree đã merge, sạch, không còn terminal sống; xóa hàng loạt qua `git::delete_worktree` (giữ mọi kiểm tra an toàn).
3. Floating terminal (Ctrl+` overlay một shell ở project root).
4. Phím tắt chỉnh được: một registry runtime duy nhất thay cho `handleShortcut` hard-code trong `Workspace.svelte`/`App.svelte`, phát hiện xung đột, lưu override; `src/ui/settings/shortcut-catalog.ts` hiện chỉ để hiển thị.
5. Git settings section, diff comments, Markdown preview tab, Explorer duplicate/drag-move/multi-select, reopen tab tài liệu đã đóng, automations local (lịch gửi prompt tới agent terminal), workspace board/dashboard.
6. Build release (`npm run release`); chỉ khi user cho phép: đo RAM bốn omp thật. Chưa có con số RAM nào; không claim dưới 150 MiB.
7. Các mục **External** trong FEATURES.md (GitHub/GitLab, accounts, browser pane, SSH, mobile, voice, plugins...) cần dịch vụ/thiết bị thật: không làm giả, giữ màn hình "unavailable" có ghi chú.

## Cách chạy và kiểm chứng

- Kiểm tra: `npm run check`, `npm run build`, `cd src-tauri && cargo test --locked` (47 tests).
- Smoke native cô lập: `cargo build --locked` (debug exe nạp Vite) → `npm run dev` (port 1420) → chạy exe với `MINNOW_DATA_DIR=.smoke/terminal-state` và `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9226` (xem `.smoke/LaunchTerminalCutover.ps1`). Điều khiển bằng Playwright `connect_over_cdp`; mẫu: `.smoke/*_smoke.py` (untracked). Đóng app bằng `.smoke/Close.ps1 -AppPid <pid>`, chỉ dừng process Vite mình mở.
- Agent trong smoke là PTY fixture (`.smoke/pty-fixture.exe`), không phải omp thật.

## Quy tắc

- `.gitignore` có thay đổi của user (chỉ còn `*.log`): giữ nguyên, không commit. Không `git add -A`; stage whitelist. `_ref`, `.smoke`, `node_modules`, `dist`, `target` untracked.
- Không đóng Minnow release/Orca chính của user. Không dùng `navigator.clipboard.readText()` trong smoke (WebView2 bật hộp quyền chặn trang).
- Trong Bash tool, nhiều heredoc trong một lệnh hay lỗi parse: viết script Python ra file rồi chạy.
- Commit từng milestone đã chạy thật; báo đúng phần đã exercise và giới hạn. Cập nhật FEATURES.md/TODO.md/SESSION_HANDOFF.md theo trạng thái thật.
