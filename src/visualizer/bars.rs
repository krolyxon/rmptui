//! Turning spectra into smoothly animated bars.

use std::time::Instant;

use super::AudioFrame;

/// Bar heights (0.0–1.0) with gravity falloff between frames.
#[derive(Debug)]
pub struct Bars {
    heights: Vec<f32>,
    last_update: Instant,
}

impl Default for Bars {
    fn default() -> Self {
        Self {
            heights: Vec::new(),
            last_update: Instant::now(),
        }
    }
}

/// Decibel range mapped onto the bar height.
const FLOOR_DB: f32 = -60.0;
const CEILING_DB: f32 = 0.0;

impl Bars {
    /// Updates `count` bars from `spectrum`, covering `min_hz..max_hz` on a
    /// logarithmic scale. `falloff` is in heights per second.
    pub fn update(
        &mut self,
        spectrum: &AudioFrame,
        count: usize,
        min_hz: f32,
        max_hz: f32,
        falloff: f32,
    ) -> &[f32] {
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32().min(0.25);
        self.last_update = now;
        self.heights.resize(count, 0.0);
        let targets = band_levels(spectrum, count, min_hz, max_hz);
        for (height, target) in self.heights.iter_mut().zip(targets) {
            *height = if target >= *height {
                // Rise quickly but not instantly, to avoid flicker.
                *height + (target - *height) * 0.7
            } else {
                (*height - falloff * dt).max(target)
            };
        }
        &self.heights
    }
}

/// Levels (0.0–1.0) of `count` log-spaced bands.
pub fn band_levels(spectrum: &AudioFrame, count: usize, min_hz: f32, max_hz: f32) -> Vec<f32> {
    let bins = &spectrum.magnitudes;
    if count == 0 || bins.is_empty() || spectrum.bin_hz <= 0.0 {
        return vec![0.0; count];
    }
    let max_hz = max_hz.min(spectrum.bin_hz * (bins.len() - 1) as f32);
    let ratio = (max_hz / min_hz).max(1.0);
    (0..count)
        .map(|band| {
            let lo = min_hz * ratio.powf(band as f32 / count as f32);
            let hi = min_hz * ratio.powf((band + 1) as f32 / count as f32);
            let first = (lo / spectrum.bin_hz).floor() as usize;
            let last = ((hi / spectrum.bin_hz).ceil() as usize)
                .max(first + 1)
                .min(bins.len());
            let peak = bins[first.min(bins.len() - 1)..last]
                .iter()
                .copied()
                .fold(0.0f32, f32::max);
            // Tilt +3 dB per octave so that treble is not dwarfed by bass.
            let center = (lo * hi).sqrt();
            let tilt = 3.0 * (center / 1000.0).log2();
            let db = 20.0 * peak.max(1e-9).log10() + tilt;
            ((db - FLOOR_DB) / (CEILING_DB - FLOOR_DB)).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spectrum_with_peak(hz: f32) -> AudioFrame {
        let bin_hz = 44_100.0 / 2048.0;
        let mut magnitudes = vec![0.0; 1025];
        magnitudes[(hz / bin_hz).round() as usize] = 1.0;
        AudioFrame {
            magnitudes,
            bin_hz,
            ..AudioFrame::default()
        }
    }

    #[test]
    fn peak_lands_in_the_right_band() {
        let levels = band_levels(&spectrum_with_peak(1000.0), 20, 40.0, 16_000.0);
        let loudest = levels
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        // 1 kHz sits a bit past the middle of 40 Hz–16 kHz on a log scale.
        assert!((10..=12).contains(&loudest), "loudest band {loudest}");
        assert!(levels[0] == 0.0);
    }

    #[test]
    fn silence_and_empty_input_give_zero() {
        assert_eq!(
            band_levels(&AudioFrame::default(), 4, 40.0, 16_000.0),
            [0.0; 4]
        );
    }

    #[test]
    fn bars_fall_gradually() {
        let mut bars = Bars::default();
        let loud = spectrum_with_peak(1000.0);
        let peak = bars.update(&loud, 20, 40.0, 16_000.0, 1.0).to_vec();
        let after = bars.update(&AudioFrame::default(), 20, 40.0, 16_000.0, 1.0);
        let i = peak.iter().position(|&h| h > 0.0).unwrap();
        assert!(after[i] > 0.0 && after[i] <= peak[i]);
    }
}
