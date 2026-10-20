## rmptui-v0.2.0

A rewrite from scratch with a new architecture modelled on ncmpcpp and rmpc.

- New screens: Media Library (artists → albums → songs), Tag Editor,
  ncmpcpp-style Visualizer (wave, filled wave, spectrum and ellipse, in
  stereo, from an MPD fifo output) and synced Lyrics (local `.lrc`
  files, cache and LRCLIB), plus a lyrics pane on the queue.
- Browser shows stored playlists at the bottom of the root folder;
  playlist editor, queue and browser keep every 0.1 feature.
- MPD I/O runs in background tasks; key presses never wait for the server,
  and the client reconnects automatically. Unix sockets are supported.
- Marks (`v`) for acting on several items, fuzzy search on every list,
  confirmation before clearing the queue or deleting playlists, a key help
  popup (`?`) and mouse support (select, double-click, wheel, tabs, seek).
- New configuration format with per-context key bindings, a theme, queue
  columns and lyrics/visualizer settings. The documented default file is the
  single source of truth; partial files keep all other defaults. 0.1 config
  files are detected and rejected with instructions.
- ncmpcpp-style interface: screen title and volume on top, column headers,
  folders spanning the whole row, reverse-video selection and a
  `Playing: "Title" by Artist in Album` status line under the progress bar.
- The tag editor follows ncmpcpp (directories, tag types, tags) and can also
  rename files and fix letter case.
- The music directory is read from the local `mpd.conf` when MPD runs on the
  same machine.
- `c` clears the queue from every screen.
- New `--config` and `--print-default-config` options; `RMPTUI_LOG` sets
  the log level.

## rmptui-v0.1.7

- Replace the outdated MPD crate with an asynchronous native MPD TCP protocol
  client and `idle`-based server updates.
- Preserve and improve browser search with fuzzy result cycling using `n`/`N`,
  and render directory names across the full browser row.
- Add a first-run TOML configuration for MPD connection details, playback
  adjustments, and configurable keybindings, with documented defaults and
  environment-variable overrides.
- Add playlist management in the Playlists view: focus playlist contents,
  load playlists, play individual tracks, remove/reorder songs, and rename or
  delete playlists.
- Add a local install/uninstall script for the release executable.
- Remove dmenu support and document the async architecture, configuration,
  keybindings, installation, and playlist controls.
- Refactor application state and browser entries, remove unused helpers, and
  improve terminal restoration and MPD address handling.
- Add and update tests for MPD protocol commands, configuration, search,
  browser rendering, and address formatting.

## rmptui-v0.1.6
    - Add songs to new playlist feature
    - Code refactoring
    - fix #9
