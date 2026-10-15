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
