## rmptui - A MPD client in Rust
![LOC](https://tokei.rs/b1/github/krolyxon/rmptui?category=code)
![Release](https://img.shields.io/github/v/release/krolyxon/rmptui?color=%23c694ff)
[![GitHub Downloads](https://img.shields.io/github/downloads/krolyxon/rmptui/total.svg?label=GitHub%20downloads)](https://github.com/krolyxon/rmptui/releases)

rmptui is a minimal tui mpd client made with rust.

## rmptui in action
![](https://raw.githubusercontent.com/krolyxon/rmptui/master/assets/ss.png)

### Keys
| Key                       | Action                                          |
| ---                       | ---                                             |
| `q`/`Ctr+C`               | Quit                                            |
| `p`                       | Toggle pause                                    |
| `+`/'='                   | Increase volume                                 |
| `-`                       | Decrease volume                                 |
| `D`                       | Get dmenu prompt                                |
| `j`/`Down`                | Scroll down                                     |
| `k`/`Up`                  | Scroll up                                       |
| `J`                       | Swap highlighted song with next one             |
| `K`                       | Swap highlighted song with previous one         |
| `l`/`Right`               | Add song to playlist or go inside the directory |
| `h`/`Left`                | Go back to previous directory                   |
| `Tab`                     | Cycle through tabs                              |
| `1`                       | Go to queue                                     |
| `2`                       | Go to directory browser                         |
| `3`                       | Go to playlists view                            |
| `Enter`/`l`/`Right`       | Add song/playlist to current playlist           |
| `a`                       | Append the song to current playing queue        |
| `Space`/`BackSpace`       | Delete the highlighted song from queue          |
| `f`                       | Go forwards                                     |
| `b`                       | Go backwards                                    |
| `>`                       | Play next song from queue                       |
| `<`                       | Play previous song from queue                   |
| `U`                       | Update the MPD database                         |
| `r`                       | Toggle repeat                                   |
| `z`                       | Toggle random                                   |
| `/`                       | Search                                          |
| `g`                       | Go to top of list                               |
| `G`                       | Go to bottom of list                            |

### Prerequisites
- [MPD](https://wiki.archlinux.org/title/Music_Player_Daemon) installed and configured.
- [dmenu](https://tools.suckless.org/dmenu/) (optional)

### TODO
- [x] fix performance issues
- [x] improvements on queue control
- [x] add to playlists
- [x] search for songs
- [x] Human readable time format
- [x] metadata based tree view
- [x] view playlist
- [x] change playlist name
- [ ] add lyrics fetcher
