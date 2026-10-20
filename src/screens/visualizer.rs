//! Screen 6: music visualizer fed by MPD's fifo output, with ncmpcpp's
//! styles: wave, filled wave, spectrum and ellipse.

use std::sync::Arc;

use ratatui::{Frame, buffer::Buffer, layout::Rect, style::Color, text::Line};
use tokio::sync::watch;

use super::{Ctx, Screen};
use crate::{
    action::Action,
    config::{Config, VisualizerConfig, VisualizerStyle},
    ui,
    visualizer::{AudioFrame, Bars},
};

const EIGHTHS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub struct VisualizerScreen {
    frames: watch::Receiver<Arc<AudioFrame>>,
    bars: Bars,
    style: VisualizerStyle,
}

impl VisualizerScreen {
    pub fn new(config: &Config, frames: watch::Receiver<Arc<AudioFrame>>) -> Self {
        Self {
            frames,
            bars: Bars::default(),
            style: config.visualizer.style,
        }
    }

    fn draw_waves<'a>(&self, buffer: &mut Buffer, area: Rect, audio: &'a AudioFrame, ctx: &Ctx) {
        let config = &ctx.config.visualizer;
        let colors = &ctx.theme().visualizer;
        let filled = self.style == VisualizerStyle::WaveFilled;
        // Like ncmpcpp, show the audio of one frame.
        let window = (config.sample_rate / config.framerate) as usize;
        let recent = |samples: &'a [f32]| &samples[samples.len().saturating_sub(window)..];
        if config.stereo && config.channels == 2 {
            let top = Rect {
                height: area.height / 2,
                ..area
            };
            let bottom = Rect {
                y: area.y + top.height,
                height: area.height - top.height,
                ..area
            };
            draw_wave(
                buffer,
                top,
                recent(&audio.left),
                filled,
                &config.wave_symbol,
                colors,
            );
            draw_wave(
                buffer,
                bottom,
                recent(&audio.right),
                filled,
                &config.wave_symbol,
                colors,
            );
        } else {
            let mono: Vec<f32> = recent(&audio.left)
                .iter()
                .zip(recent(&audio.right))
                .map(|(l, r)| (l + r) / 2.0)
                .collect();
            draw_wave(buffer, area, &mono, filled, &config.wave_symbol, colors);
        }
    }

    fn draw_spectrum(&mut self, buffer: &mut Buffer, area: Rect, audio: &AudioFrame, ctx: &Ctx) {
        let config = &ctx.config.visualizer;
        let stride = config.bar_width + config.bar_gap;
        let count = usize::from((area.width + config.bar_gap) / stride);
        let heights = self.bars.update(
            audio,
            count,
            config.min_frequency,
            config.max_frequency,
            config.falloff,
        );
        let colors = &ctx.theme().visualizer;
        let rows = area.height;
        // Center the bars horizontally.
        let used = (count as u16 * stride).saturating_sub(config.bar_gap);
        let left = area.x + area.width.saturating_sub(used) / 2;
        for (i, &height) in heights.iter().enumerate() {
            let eighths = (height * f32::from(rows) * 8.0).round() as u32;
            for row in 0..rows {
                let filled = eighths.saturating_sub(u32::from(row) * 8).min(8) as usize;
                if filled == 0 {
                    break;
                }
                let y = area.bottom() - 1 - row;
                let color = gradient(colors, f32::from(row) / f32::from(rows));
                for dx in 0..config.bar_width {
                    let x = left + i as u16 * stride + dx;
                    if x < area.right() {
                        buffer[(x, y)].set_symbol(EIGHTHS[filled]).set_fg(color);
                    }
                }
            }
        }
    }
}

impl Screen for VisualizerScreen {
    fn title(&self, _ctx: &Ctx) -> Line<'static> {
        Line::from("Music visualizer")
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let audio = Arc::clone(&self.frames.borrow_and_update());
        if let Some(error) = &audio.error {
            draw_setup_help(frame, area, error, &ctx.config.visualizer, ctx);
            return;
        }
        if area.width == 0 || area.height == 0 {
            return;
        }
        let buffer = frame.buffer_mut();
        match self.style {
            VisualizerStyle::Wave | VisualizerStyle::WaveFilled => {
                self.draw_waves(buffer, area, &audio, ctx);
            }
            VisualizerStyle::Spectrum => self.draw_spectrum(buffer, area, &audio, ctx),
            VisualizerStyle::Ellipse => draw_ellipse(
                buffer,
                area,
                &audio,
                &ctx.config.visualizer.wave_symbol,
                &ctx.theme().visualizer,
            ),
        }
    }

    fn handle_action(&mut self, action: Action, ctx: &mut Ctx) -> bool {
        match action {
            Action::CycleVisualizer => {
                self.style = self.style.next();
                ctx.notify(format!("Visualizer: {}", self.style.name()));
                true
            }
            _ => false,
        }
    }
}

fn draw_setup_help(
    frame: &mut Frame,
    area: Rect,
    error: &str,
    config: &VisualizerConfig,
    ctx: &Ctx,
) {
    let fifo = config.fifo.display().to_string();
    ui::draw_placeholder(
        frame,
        area,
        &[
            error,
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
        ctx.theme(),
    );
}

/// Draws one channel across `area`, centered vertically. A plain wave joins
/// each point to the previous column's; a filled one reaches the center line.
fn draw_wave(
    buffer: &mut Buffer,
    area: Rect,
    samples: &[f32],
    filled: bool,
    symbol: &str,
    colors: &[Color],
) {
    if area.width == 0 || area.height == 0 || samples.is_empty() {
        return;
    }
    let center = center_row(area.height);
    let mut previous = None;
    for x in 0..area.width {
        let sample = column_average(samples, usize::from(x), usize::from(area.width));
        let row = sample_row(sample, area.height);
        let (from, to) = if filled {
            span(center, row)
        } else {
            span(previous.unwrap_or(row), row)
        };
        previous = Some(row);
        for y in from..=to {
            let distance = f32::from(y.abs_diff(center)) / f32::from(center.max(1));
            buffer[(area.x + x, area.y + y)]
                .set_symbol(symbol)
                .set_fg(gradient(colors, distance));
        }
    }
}

/// Plots left against right channel samples, one point per sample.
fn draw_ellipse(
    buffer: &mut Buffer,
    area: Rect,
    audio: &AudioFrame,
    symbol: &str,
    colors: &[Color],
) {
    let half_w = f32::from(area.width.saturating_sub(1)) / 2.0;
    let half_h = f32::from(area.height.saturating_sub(1)) / 2.0;
    for (&l, &r) in audio.left.iter().zip(&audio.right) {
        let (l, r) = (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0));
        let x = (half_w + l * half_w).round() as u16;
        let y = (half_h - r * half_h).round() as u16;
        let distance = (l * l + r * r).sqrt().min(1.0);
        buffer[(area.x + x, area.y + y)]
            .set_symbol(symbol)
            .set_fg(gradient(colors, distance));
    }
}

/// The mean of the samples that fall into column `x` of `width`.
fn column_average(samples: &[f32], x: usize, width: usize) -> f32 {
    let start = x * samples.len() / width;
    let end = ((x + 1) * samples.len() / width).max(start + 1);
    let column = &samples[start..end.min(samples.len())];
    column.iter().sum::<f32>() / column.len() as f32
}

/// The row of the zero line in an area `height` rows tall.
fn center_row(height: u16) -> u16 {
    height.saturating_sub(1) / 2
}

/// The row a sample in `-1.0..=1.0` lands on; positive values go up.
fn sample_row(sample: f32, height: u16) -> u16 {
    let last = f32::from(height.saturating_sub(1));
    let row = ((1.0 - sample.clamp(-1.0, 1.0)) / 2.0 * last).round();
    row as u16
}

/// The rows from `a` to `b`, lowest first.
fn span(a: u16, b: u16) -> (u16, u16) {
    (a.min(b), a.max(b))
}

/// The theme color for `level` in `0.0..=1.0`, from the first color up.
fn gradient(colors: &[Color], level: f32) -> Color {
    let index = (level.clamp(0.0, 1.0) * colors.len() as f32) as usize;
    colors
        .get(index.min(colors.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_map_to_rows() {
        assert_eq!(sample_row(1.0, 11), 0);
        assert_eq!(sample_row(0.0, 11), 5);
        assert_eq!(sample_row(-1.0, 11), 10);
        assert_eq!(sample_row(5.0, 11), 0);
        assert_eq!(center_row(11), 5);
    }

    #[test]
    fn columns_average_their_samples() {
        let samples = [0.0, 1.0, 0.5, 0.5];
        assert_eq!(column_average(&samples, 0, 2), 0.5);
        assert_eq!(column_average(&samples, 1, 2), 0.5);
        // More columns than samples repeats samples instead of panicking.
        assert_eq!(column_average(&samples, 7, 8), 0.5);
    }

    #[test]
    fn plain_wave_connects_to_previous_point() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 11));
        draw_wave(
            &mut buffer,
            Rect::new(0, 0, 2, 11),
            &[1.0, -1.0],
            false,
            "●",
            &[Color::Red],
        );
        let column = |x| (0..11).filter(|&y| buffer[(x, y)].symbol() == "●").count();
        assert_eq!(column(0), 1);
        assert_eq!(column(1), 11);
    }

    #[test]
    fn filled_wave_reaches_center() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 11));
        draw_wave(
            &mut buffer,
            Rect::new(0, 0, 1, 11),
            &[1.0],
            true,
            "●",
            &[Color::Red],
        );
        let drawn: Vec<u16> = (0..11)
            .filter(|&y| buffer[(0, y)].symbol() == "●")
            .collect();
        assert_eq!(drawn, (0..=5).collect::<Vec<_>>());
    }

    #[test]
    fn gradient_picks_by_level() {
        let colors = [Color::Blue, Color::Green, Color::Red];
        assert_eq!(gradient(&colors, 0.0), Color::Blue);
        assert_eq!(gradient(&colors, 0.5), Color::Green);
        assert_eq!(gradient(&colors, 1.0), Color::Red);
    }

    #[test]
    fn styles_cycle_like_ncmpcpp() {
        let mut style = VisualizerStyle::Wave;
        let mut seen = vec![style];
        for _ in 0..4 {
            style = style.next();
            seen.push(style);
        }
        assert_eq!(
            seen,
            [
                VisualizerStyle::Wave,
                VisualizerStyle::WaveFilled,
                VisualizerStyle::Spectrum,
                VisualizerStyle::Ellipse,
                VisualizerStyle::Wave,
            ]
        );
    }
}
