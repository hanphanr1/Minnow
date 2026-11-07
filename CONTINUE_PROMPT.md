# Prompt bàn giao Minnow cho agent tiếp theo

Tiếp tục project Minnow tại `C:/Users/DELL/Documents/ORCA TAURI`, repo `https://github.com/hanphanr1/Minnow.git`, branch `main`. Đây là app Windows Tauri 2 + Rust + Svelte 5, mô phỏng UX Orca để quản lý omp CLI. Giữ ưu tiên nhẹ RAM, UI nhanh, không Electron. User đã yêu cầu tiếp tục toàn bộ các chức năng Orca còn thiếu, sau đó tạm dừng phiên trước để bàn giao; nhiệm vụ của bạn là tiếp tục từ trạng thái đã giữ lại, không viết lại project.

Đọc theo thứ tự: `SESSION_HANDOFF.md`, `TERMINAL_CUTOVER.md`, `TODO.md`, `UI_PARITY.md`, rồi các đoạn code liên quan. `REFERENCE.md`, `README.md`, `THIRD_PARTY_NOTICES.md` chứa nền tảng, build và attribution. Đừng dựa vào README cũ thay cho implementation.

## Checkpoint và mức độ hoàn thành

- Mốc code cuối đã kiểm chứng và push: `1f4746c60c46cac2fa7cb6c0459d82672c1b7774`. Trước đó có fix ConPTY `f122314`; UI shell/evidence `0997e90`.
- Mốc 1f4746c có native files/search/edit/save, file watcher, Git status/stage/unstage/commit/history/diff, project add/create/local clone, bảo vệ dirty text, CRLF và native close, profile WebView2 riêng cho smoke. Frontend check/build và native build thành công; 27 native tests pass. Native runtime đã chứng minh các luồng này bằng dữ liệu fixture thật. Chưa build release mới cho milestone này.
- Full Orca parity chưa xong. Đừng đánh dấu xong dựa trên screenshot/menu hoặc fake provider/runtime. Danh sách đầy đủ và các prerequisite ở TODO/UI_PARITY.
- Đo RAM với bốn omp thật vẫn đang tạm dừng theo user. Chưa có số RAM thực tế; smoke dùng PTY fixture, không được gọi đó là bốn phiên omp inference hay claim dưới 150 MiB.

## Việc đang dở cần hoàn tất trước

Tách Git worktree identity (`Worker`) khỏi từng PTY (`TerminalSession`): nhiều terminal/tab/pane chung một worktree. New/split không được tạo worktree mới.

Working tree có sáu source files đã sửa nhưng chưa kiểm chứng/commit: `src/types.ts`, `src/api.ts`, `src/Terminal.svelte`, `src/ui/PaneTree.svelte`, `src/ui/WorkspaceTabStrip.svelte`, `src/ui/workspace-layout.ts`. Chúng đã chuyển một phần sang terminalId và layout v3 per-worker.

`src/ui/Workspace.svelte` chưa migrate, còn import/call helper cũ nên hiện tại không compile được với layout mới. `src/App.svelte` chưa tích hợp state/events/props mới. Rust `model.rs`, `storage.rs`, `pty.rs`, `lib.rs` hoàn toàn chưa migrate, vẫn dùng workerId. Đây là trạng thái bàn giao cố ý giữ dở, không phải một milestone chạy được.

Có bản backup diff sáu file: `.smoke/checkpoints/terminal-cutover-paused.patch`; SHA-256 và kiểm chứng reverse-check ở SESSION_HANDOFF. Diff đã nằm trong working tree, không apply lại. Không revert hoặc commit như feature hoàn tất khi chưa tích hợp/kiểm chứng.

Làm theo contract TERMINAL_CUTOVER.md, với một integration owner cho shared APIs:
1. Native: TerminalSession + SQLite migration idempotent giữ old OMP session paths; mỗi terminal tự sở hữu process/ring/lease/desired_running. Các PTY commands dùng terminalId. Worker status aggregate, stop/restart/merge/delete phải xét toàn bộ terminal thuộc worktree. Migrate mọi caller, không để compatibility alias.
2. Frontend Workspace: terminal helpers/props, layout persistence v3 scoped project+worker, migrate layout cũ đúng nghĩa; async close/reopen/split, giữ tab geometry/menus/shortcuts. Close một pane chỉ dừng terminal của nó. New Terminal mặc định shell; New Agent Terminal dùng omp. Reopen giữ terminal identity/session.
3. App: subscribe/queue terminal events trước bootstrap, tách terminal collection khỏi workers, lọc theo worker, không gán terminalId vào activeWorkerId, nối đúng callback interface trong contract.
4. Close giữ durable terminal record: `terminal-updated` với closed=true; `terminal-removed` chỉ dành cho xóa record thật. Không làm mất reopen history do hiểu sai hai event.
5. Windows cwd: đọc nguồn đã dẫn trong contract. Orca không có process-cwd lookup native Windows; dùng OSC7 đã quan sát, rồi cached launch cwd/root fallback. Parser bounded, BEL/ST và chunk boundaries; không bịa PEB/lsof, không ép profile/prompt mới để fake parity.

Sau tích hợp mới chạy frontend check/build và Rust check/tests/build, rồi smoke native desktop thực sự. Bắt buộc chứng minh ít nhất ba PID terminal độc lập trong một worktree, new/split không tăng worktree count, cwd split từ OSC7 khi shell thực sự emit, I/O độc lập, close chỉ đúng process, merge/delete safety, reopen cùng identity và cold resume không relaunch terminal đã stop/close. Cập nhật selector `data-pane-worker` thành `data-pane-terminal` nơi phù hợp. Test pass không thay thế native smoke.

## Bước tiếp sau terminal cutover

Explorer cần Names/Contents switch cố định, Find files toàn workspace theo ignore/budgets và Git decorations. Chưa có implementation hay command `find_workspace_files`; đó mới là kế hoạch. Reuse traversal hiện có thay vì tạo walker thứ hai. File/diff cần trở thành tab trong workspace groups, không thay toàn bộ center. Tiếp tục các nhóm còn lại theo TODO/UI_PARITY; không tự thu hẹp mục tiêu full parity.

## Quy tắc giữ trạng thái

- `.gitignore` đang có thay đổi không thuộc feature, quan sát chỉ còn `*.log`; giữ nguyên. Không `git add -A`: `_ref`, `.smoke`, node_modules, dist, Rust target có thể hiện untracked. Stage whitelist các file thuộc việc mình làm.
- Không xóa reference binaries/artwork/captures/SQLite/session files. Giữ attribution Orca/font/icon.
- Không đóng app release/main Orca của user. Smoke dùng MINNOW_DATA_DIR, WebView2 profile và cổng CDP riêng. Các worker triển khai, Vite service và isolated reference runtime của phiên trước đã dừng; không giả định service/CDP cũ còn chạy.
- Runtime proof mới phải dùng code hiện tại, không gắn kết quả 27 tests hoặc ảnh milestone cũ vào phần terminal migration mới.
- Commit từng milestone thực sự chạy được; docs-only checkpoint không phải feature hoàn tất. Báo đúng phần đã exercise, thiếu prerequisite và giới hạn. Không hỏi lại những thứ có sẵn trong repo/tools.
