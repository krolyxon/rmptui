# Copilot instructions for rmptui

## Build, test, and lint

- `cargo build` — compile the application.
- `cargo run` — run against MPD at `127.0.0.1:6600` by default.
- `cargo fmt --check` — check Rust formatting; use `cargo fmt` to format.
- `cargo test` — run all unit tests.
- `cargo test connection::tests::parses_quoted_mpd_values_and_escapes` — run one named test. Replace the filter with a test name or module path to target another test.
- `cargo clippy --all-targets -- -D warnings` — lint all targets and treat warnings as errors.

## Architecture

- `src/main.rs` owns the Tokio runtime, reads `MPD_HOST`/`MPD_PORT`, initializes the terminal, and selects between terminal input, redraw ticks, and MPD change notifications. A separate `MpdClient` connection blocks on MPD `idle`, then sends a fresh `MpdSnapshot` to the UI; the app's command connection stays available for user actions.
- `src/config.rs` loads or creates the TOML configuration at `$CONFIG_HOME/rmptui/config.toml`, falling back to `$XDG_CONFIG_HOME` and then `~/.config`. Its generated TOML documents every option and keybinding. MPD host/port settings can be overridden with `MPD_HOST` and `MPD_PORT`.
- `src/connection.rs` implements MPD's line-oriented text protocol over Tokio TCP, including command quoting, response/error parsing, and conversion of MPD responses into app models. Keep protocol I/O asynchronous. The client processes a command's complete response before issuing another command on that connection.
- `src/app.rs` owns UI/application state and coordinates queue, browser, and playlist operations. `src/browser.rs` represents rows as `BrowserEntry::Directory` or `BrowserEntry::Song`, keeping song metadata with its row. Browser paths are MPD library-relative paths, not local filesystem paths. The Playlists view has separate list/song focus and supports stored-playlist track deletion and reordering through MPD's `playlistdelete` and `playlistmove` commands.
- `src/event_handler/handler.rs` maps Crossterm events to application operations. `src/ui.rs` renders the current app state with Ratatui; it should not perform MPD I/O. `src/tui.rs` owns terminal mode setup, drawing, and restoration.
- `src/list.rs` contains shared list cursor behavior. `src/utils.rs` provides the built-in audio-extension check used by the browser and directory operations.

## Repository conventions

- Keep the MPD protocol models and wire-format parsing in `connection.rs`; UI and application code should use those models rather than parse raw MPD responses.
- Use the crate-level `AppResult<T>` for errors crossing module boundaries. Avoid duplicating state that can be derived from the MPD status or other canonical model data.
- Keep configuration defaults and validation in `config.rs`, and read tunable behavior from `App.config` rather than adding new hard-coded runtime values. Route keyboard actions through `App.config.keybindings`; keep README controls and the generated defaults in sync. Text typed into search/name fields is content, not a keybinding, except for configured input-editing actions.
- When adding or changing config options, update all three: the runtime default/config validation, descriptive comments in the generated first-run TOML, and the README example/explanation. Existing partial configs should continue receiving defaults for newly added keybindings.
- Pass MPD library paths as protocol arguments. The MPD command writer quotes and escapes each argument, so do not construct raw command strings from user-provided names or paths.
- Preserve async boundaries through event handling and app operations; do not introduce blocking socket reads, polling threads, or synchronous terminal polling.
- Browser `/` search retains all positive-scoring fuzzy matches in result order. `Enter` returns to normal navigation while retaining browser results; `n` and `N` cycle them. Starting a new search or changing directories resets those results.
- In the Playlists view, `Left`/`h` and `Right`/`l` switch focus between playlist names and song contents. `Space`/`Enter` load a selected playlist from the playlist list; when song contents are focused, `Enter` plays that song, `d` deletes it from the stored playlist, and `J`/`K` reorder it. `D` deletes the selected stored playlist and `R` renames it. Keep these actions context-aware and all defaults documented in the README/config.
- When changing key behavior or features, keep the controls documented in `README.md` consistent with `event_handler/handler.rs`.
- `AppResult<T>` is `Send + Sync` because application errors cross Tokio task boundaries.
