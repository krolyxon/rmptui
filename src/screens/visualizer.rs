//! Screen 6: spectrum visualizer fed by MPD's fifo output.

use std::sync::Arc;

use ratatui::{Frame, layout::Rect};
use tokio::sync::watch;

use super::{Ctx, Screen};
use crate::{
    action::Action,
    ui,
    visualizer::{Bars, Spectrum},
};

const EIGHTHS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub struct VisualizerScreen {
    spectrum: watch::Receiver<Arc<Spectrum>>,
    bars: Bars,
}

impl VisualizerScreen {
    pub fn new(spectrum: watch::Receiver<Arc<Spectrum>>) -> Self {
        Self {
            spectrum,
            bars: Bars::default(),
        }
    }
}

impl Screen for VisualizerScreen {
    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = ctx.theme();
        let config = &ctx.config.visualizer;
        let title = match &ctx.mpd.current {
            Some(song) => format!(" Visualizer · {} ", ui::song_label(song)),
            None => " Visualizer ".to_owned(),
        };
        let block = ui::pane(title, true, theme);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let spectrum = Arc::clone(&self.spectrum.borrow_and_update());
        if let Some(error) = &spectrum.error {
            let fifo = config.fifo.display().to_string();
            ui::draw_placeholder(
                frame,
                inner,
                &[
                    error.as_str(),
                    "",
                    "Add a fifo output to mpd.conf and restart MPD:",
                    "",
                    "audio_output {",
                    "    type   \"fifo\"",
                    "    name   \"Visualizer feed\"",
                    &format!("    path   \"{fifo}\""),
                    "    format \"44100:16:2\"",
                    "}",
                ],
                theme,
            );
            return;
        }
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let stride = config.bar_width + config.bar_gap;
        let count = usize::from((inner.width + config.bar_gap) / stride);
        let heights = self.bars.update(
            &spectrum,
            count,
            config.min_frequency,
            config.max_frequency,
            config.falloff,
        );
        let colors = &theme.visualizer;
        let rows = inner.height;
        // Center the bars horizontally.
        let used = (count as u16 * stride).saturating_sub(config.bar_gap);
        let left = inner.x + inner.width.saturating_sub(used) / 2;
        let buffer = frame.buffer_mut();
        for (i, &height) in heights.iter().enumerate() {
            let eighths = (height * f32::from(rows) * 8.0).round() as u32;
            for row in 0..rows {
                let filled = eighths.saturating_sub(u32::from(row) * 8).min(8) as usize;
                if filled == 0 {
                    break;
                }
                let y = inner.bottom() - 1 - row;
                let level = usize::from(row) * colors.len() / usize::from(rows);
                let color = colors[level.min(colors.len() - 1)];
                for dx in 0..config.bar_width {
                    let x = left + i as u16 * stride + dx;
                    if x < inner.right() {
                        buffer[(x, y)].set_symbol(EIGHTHS[filled]).set_fg(color);
                    }
                }
            }
        }
    }

    fn handle_action(&mut self, _action: Action, _ctx: &mut Ctx) -> bool {
        false
    }
}
