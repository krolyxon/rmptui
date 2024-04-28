## rmptui (Rust Music Player TUI(💀))
A MPD client in Rust

## rmptui in action
![](https://raw.githubusercontent.com/krolyxon/rmptui/master/assets/ss.png)

### Keys
- `q` OR `Ctr+C` to quit
- `p` to toggle pause
- `+` to increase volume
- `-` to decrease volume
- `D` to get dmenu prompt
- `j` to scroll down
- `k` to scroll up
- `l` add song to playlist or go inside the directory
- `h` to go back to previous directory
- `Tab` to cycle through tabs
- `1` to go to directory tree
- `2` to go to current playing queue
- `3` to go to playlists view
- `Enter` to add song/playlist to current playlist
- `a` to append the song to current playing queue
- `Space`/`BackSpace` to delete the highlighted song from queue
- `f` to go forwards
- `b` to go backwards
- `>` to play next song from queue
- `<` to play previous song from queue
- `U` to update the MPD database
- `r` to toggle repeat
- `z` to toggle random
- `/` to search
- `g` to go to top of list
- `G` to go to bottom of list

### TODO
- [x] fix performance issues
- [ ] improvements on queue control
- [x] add to playlists
- [ ] view playlist
- [ ] change playlist name
- [x] search for songs
- [ ] metadata based tree view
- [x] Humantime format
