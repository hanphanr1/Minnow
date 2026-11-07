# Prompt bàn giao Minnow cho agent tiếp theo

Tiếp tục project Minnow tại `C:/Users/DELL/Documents/ORCA TAURI`, repo `https://github.com/hanphanr1/Minnow.git`, branch `main`. Đây là app Windows Tauri 2 + Rust + Svelte 5, mô phỏng UX Orca để quản lý omp CLI. Giữ ưu tiên nhẹ RAM, UI nhanh, không Electron. Không viết lại project; tiếp tục từ trạng thái hiện tại.

Đọc theo thứ tự: `SESSION_HANDOFF.md`, `TERMINAL_CUTOVER.md`, `TODO.md`, `UI_PARITY.md`, rồi code liên quan. Đừng dựa vào README cũ thay cho implementation.

## Trạng thái

- Terminal cutover đã xong và đã chạy thật trên desktop native: `Worker` là Git worktree, `TerminalSession` là PTY độc lập trong worktree. New/split không tạo worktree. Bằng chứng và giới hạn chính xác ở mục "Terminal cutover evidence" trong SESSION_HANDOFF.md. Native 40 tests pass, frontend check/build sạch.
- Chưa làm: build release mới, đo RAM với bốn omp thật (đang tạm dừng theo user; không được claim dưới 150 MiB từ fixture), kiểm tra menu "New Agent Terminal", Ctrl+T từ file/diff view, flood/ring regression sau cutover.
- Full Orca parity chưa xong. Đừng đánh dấu xong dựa trên screenshot/menu hoặc fake provider/runtime. Danh sách đầy đủ ở TODO/UI_PARITY.

## Bước tiếp theo

Explorer cần Names/Contents switch cố định, Find files toàn workspace theo ignore/budgets và Git decorations. Chưa có command `find_workspace_files`; reuse traversal trong `workspace_search.rs` thay vì tạo walker thứ hai. File/diff cần thành tab trong workspace groups, không thay toàn bộ center. Sau đó tiếp tục các nhóm còn lại theo TODO/UI_PARITY; không tự thu hẹp mục tiêu full parity.

## Quy tắc giữ trạng thái

- `.gitignore` có thay đổi không thuộc feature (chỉ còn `*.log`); giữ nguyên. Không `git add -A`: `_ref`, `.smoke`, node_modules, dist, Rust target hiện untracked. Stage whitelist file thuộc việc của bạn.
- Không xóa reference binaries/artwork/captures/SQLite/session files. Giữ attribution Orca/font/icon.
- Không đóng app release/main Orca của user. Smoke dùng MINNOW_DATA_DIR, WebView2 profile và cổng CDP riêng (xem Runtime hygiene). Debug exe cần Vite dev server (port 1420); dừng đúng process mình đã mở.
- Runtime proof mới phải dùng code hiện tại. Test pass không thay thế native smoke.
- Commit từng milestone thực sự chạy được; báo đúng phần đã exercise, thiếu prerequisite và giới hạn. Không hỏi lại những thứ có sẵn trong repo/tools.
