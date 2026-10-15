use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use rust_fuzzy_search::fuzzy_search_sorted;

use crate::{
    app::{App, PlaylistFocus, SelectedTab},
    connection::VolumeStatus,
    ui::InputMode,
    AppResult,
};

pub async fn handle_key_events(key: KeyEvent, app: &mut App) -> AppResult<()> {
    if app.inputmode != InputMode::Normal {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && is_bound(app, "quit", key)
        {
            app.quit();
            return Ok(());
        }
        return handle_input_key(key, app).await;
    }
    if app.playlist_popup {
        if is_bound(app, "popup_close", key) {
            app.playlist_popup = false;
        } else if is_bound(app, "move_down", key) {
            app.append_list.next();
        } else if is_bound(app, "move_up", key) {
            app.append_list.prev();
        } else if is_bound(app, "popup_select", key) {
            app.append_selected_to().await?;
        }
        return Ok(());
    }
    if app.selected_tab == SelectedTab::DirectoryBrowser && !app.browser_search_results.is_empty() {
        if is_bound(app, "search_next", key) {
            cycle_browser_search(app, true);
            return Ok(());
        } else if is_bound(app, "search_previous", key) {
            cycle_browser_search(app, false);
            return Ok(());
        }
    }

    if is_bound(app, "quit", key) {
        app.quit();
    } else if is_bound(app, "clear_queue", key) {
        app.conn.client.clear().await?;
        app.update_queue().await?;
        app.conn.update_status().await?;
    } else if is_bound(app, "toggle_pause", key) {
        app.conn.toggle_pause().await?;
    } else if is_bound(app, "pause", key) {
        app.conn.pause().await?;
    } else if is_bound(app, "toggle_repeat", key) {
        app.conn.toggle_repeat().await?;
    } else if is_bound(app, "toggle_random", key) {
        app.conn.toggle_random().await?;
    } else if is_bound(app, "open_playlist_menu", key) {
        app.playlist_popup = true;
    } else if is_bound(app, "seek_forward", key) || is_bound(app, "seek_backward", key) {
        if let Some(position) = app.conn.status.song {
            let current = app.conn.status.elapsed.as_secs();
            let target = if is_bound(app, "seek_forward", key) {
                current.saturating_add(app.config.playback.seek_seconds)
            } else {
                current.saturating_sub(app.config.playback.seek_seconds)
            };
            app.conn
                .client
                .seek(position.pos, std::time::Duration::from_secs(target))
                .await?;
            app.conn.update_status().await?;
        }
    } else if is_bound(app, "cycle_tabs", key) {
        app.cycle_tabs();
        if app.selected_tab == SelectedTab::Playlists {
            app.refresh_playlist_view().await?;
        }
    } else if is_bound(app, "select_queue", key) {
        app.selected_tab = SelectedTab::Queue;
    } else if is_bound(app, "select_browser", key) {
        app.selected_tab = SelectedTab::DirectoryBrowser;
    } else if is_bound(app, "select_playlists", key) {
        app.selected_tab = SelectedTab::Playlists;
        app.refresh_playlist_view().await?;
    } else if is_bound(app, "next_track", key) {
        if !app.queue_list.list.is_empty() {
            app.conn.client.next().await?;
            app.update_queue().await?;
            app.conn.update_status().await?;
        }
    } else if is_bound(app, "previous_track", key) {
        if !app.queue_list.list.is_empty() {
            app.conn.client.previous().await?;
            app.update_queue().await?;
            app.conn.update_status().await?;
        }
    } else if is_bound(app, "volume_up", key) {
        app.conn
            .inc_volume(app.config.playback.volume_step as i8)
            .await?;
    } else if is_bound(app, "volume_down", key) {
        app.conn
            .dec_volume(app.config.playback.volume_step as i8)
            .await?;
    } else if is_bound(app, "toggle_mute", key) {
        match app.conn.volume_status {
            VolumeStatus::Muted(volume) => {
                app.conn.client.set_volume(volume).await?;
                app.conn.volume_status = VolumeStatus::Unmuted;
            }
            VolumeStatus::Unmuted => {
                app.conn.client.set_volume(0).await?;
                app.conn.volume_status = VolumeStatus::Muted(app.conn.status.volume);
            }
        }
        app.conn.update_status().await?;
    } else if is_bound(app, "update_database", key) {
        app.conn.client.update_database().await?;
        app.should_update_song_list = true;
        app.conn.update_status().await?;
    } else if is_bound(app, "search", key) {
        app.inputmode = InputMode::Editing;
        app.search_input.clear();
        app.search_cursor_pos = 0;
        app.browser_search_results.clear();
        app.browser_search_index = 0;
    } else if is_bound(app, "toggle_selected_item", key) {
        app.toggle_selected_item().await?;
    }

    match app.selected_tab {
        SelectedTab::Queue => handle_queue_key(key, app).await?,
        SelectedTab::DirectoryBrowser => handle_browser_key(key, app).await?,
        SelectedTab::Playlists => handle_playlists_key(key, app).await?,
    }
    Ok(())
}

fn is_bound(app: &App, action: &str, key: KeyEvent) -> bool {
    app.config.keybindings.matches(action, key)
}

async fn handle_queue_key(key: KeyEvent, app: &mut App) -> AppResult<()> {
    if is_bound(app, "move_down", key) {
        app.queue_list.next();
    } else if is_bound(app, "move_up", key) {
        app.queue_list.prev();
    } else if is_bound(app, "activate", key) {
        if app.queue_list.index < app.queue_list.list.len() {
            app.conn
                .client
                .play(Some(app.queue_list.index as u32))
                .await?;
            app.conn.update_status().await?;
        }
    } else if is_bound(app, "queue_delete", key) {
        if app.queue_list.index < app.queue_list.list.len() {
            app.conn.client.delete(app.queue_list.index as u32).await?;
            app.update_queue().await?;
            app.conn.update_status().await?;
        }
    } else if is_bound(app, "queue_move_down", key) {
        let position = app.queue_list.index;
        if position + 1 < app.queue_list.list.len() {
            app.conn
                .client
                .swap(position as u32, (position + 1) as u32)
                .await?;
            app.queue_list.index += 1;
            app.update_queue().await?;
        }
    } else if is_bound(app, "queue_move_up", key) {
        let position = app.queue_list.index;
        if position > 0 && position < app.queue_list.list.len() {
            app.conn
                .client
                .swap(position as u32, (position - 1) as u32)
                .await?;
            app.queue_list.index -= 1;
            app.update_queue().await?;
        }
    } else if is_bound(app, "go_to_top", key) {
        app.queue_list.index = 0;
    } else if is_bound(app, "go_to_bottom", key) {
        app.queue_list.index = app.queue_list.list.len().saturating_sub(1);
    }
    Ok(())
}

async fn handle_browser_key(key: KeyEvent, app: &mut App) -> AppResult<()> {
    if is_bound(app, "move_down", key) {
        app.browser.next();
    } else if is_bound(app, "move_up", key) {
        app.browser.prev();
    } else if is_bound(app, "activate", key) {
        let previous_path = app.browser.path.clone();
        app.handle_enter().await?;
        if app.browser.path != previous_path {
            app.browser_search_results.clear();
            app.search_input.clear();
        }
    } else if is_bound(app, "go_back", key) {
        app.browser.handle_go_back(&mut app.conn).await?;
        app.browser_search_results.clear();
        app.search_input.clear();
    } else if is_bound(app, "go_to_top", key) {
        app.browser.selected = 0;
    } else if is_bound(app, "go_to_bottom", key) {
        app.browser.selected = app.browser.entries.len().saturating_sub(1);
    }
    Ok(())
}

async fn handle_playlists_key(key: KeyEvent, app: &mut App) -> AppResult<()> {
    if is_bound(app, "playlist_focus_contents", key) {
        app.playlist_focus = PlaylistFocus::Songs;
        return Ok(());
    }
    if is_bound(app, "playlist_focus_list", key) {
        app.playlist_focus = PlaylistFocus::List;
        return Ok(());
    }

    if app.playlist_focus == PlaylistFocus::Songs {
        if is_bound(app, "move_down", key) {
            app.playlist_song_index = app
                .playlist_song_index
                .saturating_add(1)
                .min(app.playlist_contents.len().saturating_sub(1));
        } else if is_bound(app, "move_up", key) {
            app.playlist_song_index = app.playlist_song_index.saturating_sub(1);
        } else if is_bound(app, "go_to_top", key) {
            app.playlist_song_index = 0;
        } else if is_bound(app, "go_to_bottom", key) {
            app.playlist_song_index = app.playlist_contents.len().saturating_sub(1);
        } else if is_bound(app, "playlist_delete_song", key) {
            app.delete_selected_playlist_song().await?;
        } else if is_bound(app, "playlist_move_song_down", key) {
            app.move_selected_playlist_song(true).await?;
        } else if is_bound(app, "playlist_move_song_up", key) {
            app.move_selected_playlist_song(false).await?;
        } else if is_bound(app, "activate", key) {
            app.play_selected_playlist_song().await?;
        } else if is_bound(app, "playlist_delete", key) {
            app.delete_selected_playlist().await?;
        }
        return Ok(());
    }

    let previous_index = app.pl_list.index;
    if is_bound(app, "move_down", key) {
        app.pl_list.next();
    } else if is_bound(app, "move_up", key) {
        app.pl_list.prev();
    } else if is_bound(app, "go_to_top", key) {
        app.pl_list.index = 0;
    } else if is_bound(app, "go_to_bottom", key) {
        app.pl_list.index = app.pl_list.list.len().saturating_sub(1);
    } else if is_bound(app, "playlist_rename", key) && !app.pl_list.list.is_empty() {
        app.inputmode = InputMode::PlaylistRename;
        app.pl_newname_input.clear();
        app.pl_cursor_pos = 0;
    } else if is_bound(app, "playlist_delete", key) {
        app.delete_selected_playlist().await?;
    } else if is_bound(app, "playlist_load", key) {
        if let Some(playlist) = app.pl_list.list.get(app.pl_list.index) {
            app.conn.load_playlist(playlist).await?;
            app.update_queue().await?;
        }
    }
    if previous_index != app.pl_list.index {
        app.playlist_song_index = 0;
        app.refresh_playlist_view().await?;
    }
    Ok(())
}

async fn handle_input_key(key: KeyEvent, app: &mut App) -> AppResult<()> {
    if is_bound(app, "input_cancel", key) {
        match app.inputmode {
            InputMode::Editing => {
                app.search_input.clear();
                app.browser_search_results.clear();
                app.inputmode = InputMode::Normal;
            }
            InputMode::PlaylistRename => {
                app.pl_newname_input.clear();
                app.inputmode = InputMode::Normal;
            }
            InputMode::NewPlaylist => {
                app.pl_new_pl_input.clear();
                app.inputmode = InputMode::Normal;
            }
            InputMode::Normal => {}
        }
        app.reset_cursor();
    } else if is_bound(app, "input_backspace", key) {
        app.delete_char();
        if app.inputmode == InputMode::Editing {
            update_search_selection(app);
        }
    } else if is_bound(app, "input_cursor_left", key) {
        app.move_cursor_left();
    } else if is_bound(app, "input_cursor_right", key) {
        app.move_cursor_right();
    } else if is_bound(app, "input_submit", key) {
        match app.inputmode {
            InputMode::Editing => {
                update_search_selection(app);
                if app.selected_tab != SelectedTab::DirectoryBrowser {
                    app.search_input.clear();
                }
                if app.selected_tab == SelectedTab::Playlists {
                    app.refresh_playlist_view().await?;
                }
                app.reset_cursor();
                app.inputmode = InputMode::Normal;
            }
            InputMode::PlaylistRename => {
                if let Some(old_name) = app.pl_list.list.get(app.pl_list.index).cloned() {
                    if !app.pl_newname_input.trim().is_empty() {
                        app.conn
                            .client
                            .rename_playlist(&old_name, &app.pl_newname_input)
                            .await?;
                        app.refresh_playlists().await?;
                    }
                }
                app.pl_newname_input.clear();
                app.reset_cursor();
                app.inputmode = InputMode::Normal;
            }
            InputMode::NewPlaylist => {
                if !app.pl_new_pl_input.trim().is_empty() {
                    for song in &app.pl_new_pl_songs_buffer {
                        app.conn.add_to_playlist(&app.pl_new_pl_input, song).await?;
                    }
                    app.refresh_playlists().await?;
                }
                app.pl_new_pl_input.clear();
                app.reset_cursor();
                app.inputmode = InputMode::Normal;
            }
            InputMode::Normal => {}
        }
    } else if let KeyCode::Char(character) = key.code {
        if !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            app.enter_char(character);
            if app.inputmode == InputMode::Editing {
                update_search_selection(app);
            }
        }
    }
    Ok(())
}

fn update_search_selection(app: &mut App) {
    if app.search_input.is_empty() {
        return;
    }
    match app.selected_tab {
        SelectedTab::Queue => {
            let items: Vec<&str> = app
                .queue_list
                .list
                .iter()
                .map(|song| song.file.as_str())
                .collect();
            if let Some((best, _)) = fuzzy_search_sorted(&app.search_input, &items).first() {
                if let Some(index) = app
                    .queue_list
                    .list
                    .iter()
                    .position(|song| song.file == *best)
                {
                    app.queue_list.index = index;
                }
            }
        }
        SelectedTab::DirectoryBrowser => {
            let items: Vec<String> = app
                .browser
                .entries
                .iter()
                .map(|entry| entry.name().to_string())
                .collect();
            app.browser_search_results = fuzzy_indices(&app.search_input, &items);
            app.browser_search_index = 0;
            if let Some(index) = app.browser_search_results.first() {
                app.browser.selected = *index;
            }
        }
        SelectedTab::Playlists => {
            let items: Vec<&str> = app.pl_list.list.iter().map(String::as_str).collect();
            if let Some((best, _)) = fuzzy_search_sorted(&app.search_input, &items).first() {
                if let Some(index) = app.pl_list.list.iter().position(|name| name == *best) {
                    app.pl_list.index = index;
                }
            }
        }
    }
}

fn cycle_browser_search(app: &mut App, forward: bool) {
    let len = app.browser_search_results.len();
    if len == 0 {
        return;
    }
    app.browser_search_index = if forward {
        (app.browser_search_index + 1) % len
    } else {
        app.browser_search_index.checked_sub(1).unwrap_or(len - 1)
    };
    app.browser.selected = app.browser_search_results[app.browser_search_index];
}

fn fuzzy_indices(query: &str, items: &[String]) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    let candidates: Vec<&str> = items.iter().map(String::as_str).collect();
    let ranked = fuzzy_search_sorted(query, &candidates);
    let mut used = vec![false; items.len()];
    ranked
        .into_iter()
        .filter(|(_, score)| *score > 0.0)
        .filter_map(|(name, _)| {
            let index = items
                .iter()
                .enumerate()
                .find(|(index, item)| !used[*index] && item.as_str() == name)
                .map(|(index, _)| index)?;
            used[index] = true;
            Some(index)
        })
        .collect()
}

pub async fn handle_mouse_events(mouse: MouseEvent, app: &mut App) -> AppResult<()> {
    let previous_playlist = app.pl_list.index;
    match mouse.kind {
        MouseEventKind::ScrollUp => app.handle_scroll_up(),
        MouseEventKind::ScrollDown => app.handle_scroll_down(),
        _ => {}
    }
    if app.selected_tab == SelectedTab::Playlists && app.pl_list.index != previous_playlist {
        app.playlist_song_index = 0;
        app.refresh_playlist_view().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fuzzy_indices;

    #[test]
    fn fuzzy_search_returns_all_matches_and_keeps_duplicate_names() {
        let items = vec![
            "mr-robot-vol1".to_string(),
            "mr-robot-vol2".to_string(),
            "mr-robot-vol3".to_string(),
            "other-album".to_string(),
        ];
        let results = fuzzy_indices("robot", &items);
        assert_eq!(results.len(), 3);
        assert_eq!(
            results
                .iter()
                .map(|index| items[*index].as_str())
                .collect::<Vec<_>>(),
            vec!["mr-robot-vol1", "mr-robot-vol2", "mr-robot-vol3"]
        );
    }
}
