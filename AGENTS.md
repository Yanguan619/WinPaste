# AGENTS.md — Clipboard

A WinUI3 clipboard manager using [windows-reactor](https://github.com/microsoft/windows-rs) (reactive Rust bindings for WinUI3). Ported from Tauri to native WinUI3.

## Build & verify

```powershell
cargo check          # fastest feedback
cargo test           # rusqlite + win_clipboard tests (bundled SQLite, no system dep)
cargo build          # debug build
cargo run            # launches WinUI3 window (requires Windows App SDK runtime)
```

All commands run from repo root. No pre-build steps — build.rs handles `windows-reactor-setup::as_framework_dependent()` automatically.

## Architecture

```
main.rs → bootstrap() → app::setup::init() → App::new().render(app_ui)
                                                        │
                                                   ui::main_window::render()
```

Layered from top to bottom:

| Layer | Path | Role |
|---|---|---|
| **UI** | `src/ui/` | WinUI3 windows + controls via windows-reactor |
| **App** | `src/app/` | Init, setup, tray, window management, sticky manager |
| **Services** | `src/services/` | Clipboard monitoring pipeline (listener → monitor → ops), hotkey, paste queue |
| **Infrastructure** | `src/infrastructure/` | SQLite repos, Win32 clipboard I/O, DPAPI encryption, drag-drop, window tracking |
| **Domain** | `src/domain/` | `ClipboardEntry` model (serde) + `ClipboardEntryView` |
| **State** | `src/state/` | Atomic globals + mutex-guarded `SettingsState` |
| **Error** | `src/error.rs` | `AppError` enum with `From` impls |

### Key flows

**Clipboard monitoring**: `clipboard_listener` (Win32 `AddClipboardFormatListener`) → `clipboard_monitor` (read → dedup → save → `mpsc::Sender`) → `main_window` background thread (`mpsc::Receiver` → update `CLIPBOARD_ENTRIES`)

**Paste**: `clipboard_ops::paste_entry()` → hide window → write clipboard → set echo-prevention hash → pause monitor → focus target → `SendInput` Ctrl+V → unpause

**Hotkey**: Hidden Win32 message-only window with `RegisterHotKey(Ctrl+Shift+V)` → `WM_HOTKEY` → `window_manager::toggle_window()`

**Window show**: `position_near_cursor()` (or `position_docked()`) → `WS_EX_NOACTIVATE` → `ShowWindow(SW_SHOWNA)` → `HWND_TOPMOST`. Hide: `release_win_keys()` → `ShowWindow(SW_HIDE)` → restore last focus

## Non-obvious constraints

- **Edition 2024**: `#[macro_use]` is deprecated. Use explicit `use crate::{info, error};` to import logging macros.
- **`windows-reactor` is a git dependency** — not on crates.io. First `cargo check` pulls the entire `microsoft/windows-rs` repo.
- **Project also depends on `windows` 0.62.2 directly** for types not re-exported by windows-reactor. Add features there for new Win32 APIs.
- **Windows only** — targets `Win32` + `WinUI3`. No cross-platform intent.
- **Echo prevention** — the paste pipeline writes a content hash to `LAST_APP_SET_HASH` before writing clipboard, and the monitor checks both `LAST_APP_SET_HASH` and `LAST_APP_SET_HASH_ALT` to skip self-triggered events.
- **DPAPI encryption** — sensitive values are encrypted via `CryptProtectData`/`CryptUnprotectData` and carry the `dpapi:` prefix.
- **Clipboard format priority in monitor**: Image (CF_DIB/CF_DIBV5) > File (CF_HDROP) > Rich text (CF_HTML) > Plain text (CF_UNICODETEXT).

## UI design principles

- **Declarative WinUI3 via windows-reactor**: All UI built with `windows-reactor` components (`grid`, `vstack`, `text_block`, `ToggleSwitch`, `ComboBox`, `NumberBox`, `Slider`, `Expander`, `CommandBar`, `TitleBar`, `Image`, `scroll_viewer`, `text_box`, etc.) — no raw XAML, no direct WinUI3 interop. Every render is `fn render(cx: &mut RenderCx) -> Element`.
- **State hooks**: `cx.use_state`, `cx.use_async_state`, `cx.use_effect` for local reactive state. Global state via `state/global_state.rs` atomics and `CLIPBOARD_ENTRIES` mutex.
- **No WinUI3 code-behind**: No imperative WinUI3 API calls in the UI layer — all Win32 interop is confined to `infrastructure/windows_api/` and `app/` (window_manager, tray).
- **Window**: Appears near cursor or docked to edge. Uses `WS_EX_NOACTIVATE` so it never steals focus. Hide releases held modifier keys and restores previous foreground window.
- **Visual hierarchy**: Search bar + filter pills at top. Card-based items with source app color dot, name, relative timestamp, and type-specific preview. Type-specific rendering for text/image/file/code.
- **Color & theme**: Light and dark themes with persisted `app.color_mode` setting. Source apps color-coded. Settings use card sections with ToggleSwitch, ComboBox, NumberBox, Slider inputs persisting immediately to DB.
- **Responsive**: Default and compact mode with item/tag font size ranges adjustable per user. Window dimensions persisted between sessions.

## Implementation status

Most modules are complete. Key status:

- ✅ **All core infrastructure**: Win32 clipboard I/O (text, HTML, DIB/DIBV5, CF_HDROP, GIF/PNG), DPAPI encryption, drag-drop handler, foreground window tracking, 10 migration versions, full repository layer (clipboard, settings, sticky, tag)
- ✅ **All services**: Event-driven clipboard listener, monitor pipeline with dedup, paste operations with echo prevention, global hotkey with string parser
- ✅ **All app layer**: Setup/init, window manager (show/hide/toggle, pin, compact, edge dock, paste keystroke), tray icon with crash recovery, sticky note manager
- ✅ **All UI**: Main window (search, filter pills, card list, command bar, settings toggle, monitor polling), settings panel (ToggleSwitch/ComboBox/NumberBox/Slider/Expander), styles (light/dark themes, compact), content preview (text/image/file/code cards)
- ✅ **State**: 25+ atomic globals, `SettingsState` with DB load, paste queue state, session history
- ✅ **Domain**: `ClipboardEntry` (16 fields) + `ClipboardEntryView` (13 pre-computed fields)
- ✅ **Database**: DB init, 40+ seed defaults, text/image hash helpers, sensitive tag detection, image file saving
- ⚠️ **paste_queue** — sequential paste logic not wired up
- ⚠️ **apps.rs** — UWP app launching and default app queries are stubs
- ⚠️ **Drag-drop** — `drag_drop.rs` has handler functions but no WinUI3 drop target integration
