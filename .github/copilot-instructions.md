# Contributor and agent instructions for rmptui

## Build, test, and lint

- `cargo build` — compile the application.
- `cargo run -- --config FILE` — run with a specific config (MPD at
  `127.0.0.1:6600` by default; `MPD_HOST`/`MPD_PORT` override it).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- `cargo test mpd::protocol` — run the tests of one module; `tests/mpd_client.rs`
  exercises the client against a scripted fake MPD server.

## Architecture

- `mpd/` is a self-contained MPD client.
  - `protocol.rs`: pure command encoding (every argument quoted and escaped)
    and response parsing.
  - `connection.rs`: async TCP and Unix-socket connections.
  - `types.rs`: typed models.
  - `commands.rs`: command constructors.
  - `worker.rs`: two background tasks. The *idle* connection reports changed
    subsystems. The *command* connection runs `Request`s from an `MpdHandle`
    and answers `Query`s with `Reply`s. Both reconnect on their own.
- `app.rs` owns the state and runs one `tokio::select!` loop over terminal
  input, `AppEvent`s (MPD, lyrics, tag writes) and visualizer frames. UI code
  never awaits MPD: it sends requests and reacts to the resulting events.
- `state.rs` (`MpdState`) mirrors the server: status, current song, the queue
  (updated incrementally via `plchanges`), stored playlists, and a database
  version used to invalidate caches.
- `screens/` holds one component per screen behind the `Screen` trait (`draw`,
  `handle_action`, `on_mpd`, `on_enter`, `on_mouse`, `search`).
  - Screens receive a `Ctx` with read access to config and state, the
    `MpdHandle`, and an effect queue (`Effect`) for things only the app can do:
    notifications, popups, tag writes, lyric reloads.
  - Every screen sees every `MpdEvent`, so caches stay valid while hidden.
- `popups/` holds modal components (`Popup` trait); the topmost popup gets all
  keys. Destructive actions go through `ConfirmPopup`.
- `ui/` holds the ncmpcpp-style frame (title row, progress bar, status line), the
  virtualized searchable `ListView`, and the `LineInput` editor.
- `config/`:
  - `default.toml` is embedded and is the single source of truth for
    defaults; user files are merged over it table by table.
  - `keys.rs` parses key specs and builds the per-context `Keymap`.
- `action.rs` defines the `Action`, `Context` and `ScreenId` enums. Each action
  declares the contexts it may be bound in. Lookup order is: popup or screen
  context → `navigation` → `global`.
- `lyrics/`: LRC parser and the async lookup service (local `.lrc` → cache →
  LRCLIB). `visualizer/`: fifo reader thread with FFT, plus bar smoothing.
  `tags.rs`: lofty-based tag writing.

## Conventions

- Build MPD commands only through `mpd::commands` or `Command::arg`; never
  format raw command strings from user data. Filter expressions go through
  `protocol::filter_eq`/`filter_and`.
- Prefer position-based commands (`move`, `delete`) for actions users repeat
  quickly. They stay correct even when sent before the local queue refreshes.
- Every new action needs:
  - an `Action` variant with its contexts and description;
  - a default binding in `config/default.toml` (a test enforces this);
  - a README entry.
- Every new option needs:
  - a documented entry in `default.toml`;
  - a field in the matching config struct (`deny_unknown_fields` catches
    typos);
  - validation in `Config::validate` if it has limits;
  - README documentation.
- Screens must not block: do file and network I/O in tasks (see lyrics) or
  `spawn_blocking` (see tag writes) and report back through `AppEvent`.
