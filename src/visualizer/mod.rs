//! Reads PCM data from MPD's fifo output and computes spectra.
//!
//! A dedicated thread does the I/O and FFT while the visualizer is visible
//! and publishes each spectrum on a `watch` channel; the UI turns spectra into
//! bars (see [`bars`]).

mod bars;

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub use bars::Bars;
use realfft::{RealFftPlanner, RealToComplex};
use tokio::sync::watch;
use tracing::warn;

use crate::config::VisualizerConfig;

const FFT_SIZE: usize = 2048;

/// One analysis frame.
#[derive(Clone, Debug, Default)]
pub struct Spectrum {
    /// Normalized magnitude per FFT bin (0 Hz up to Nyquist).
    pub magnitudes: Vec<f32>,
    /// Width of one bin in Hz.
    pub bin_hz: f32,
    /// Why there is no audio, if the fifo cannot be read.
    pub error: Option<String>,
}

pub struct Visualizer {
    active: Arc<AtomicBool>,
    spectrum: watch::Receiver<Arc<Spectrum>>,
}

impl Visualizer {
    pub fn spawn(config: VisualizerConfig) -> Self {
        let active = Arc::new(AtomicBool::new(false));
        let (tx, spectrum) = watch::channel(Arc::new(Spectrum::default()));
        let flag = Arc::clone(&active);
        let spawned = thread::Builder::new()
            .name("visualizer".into())
            .spawn(move || Reader::new(config, flag, tx).run());
        if let Err(error) = spawned {
            warn!("cannot start visualizer thread: {error}");
        }
        Self { active, spectrum }
    }

    /// Starts or stops reading the fifo.
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn latest(&self) -> Arc<Spectrum> {
        Arc::clone(&self.spectrum.borrow())
    }

    /// A receiver to await new spectra on.
    pub fn subscribe(&self) -> watch::Receiver<Arc<Spectrum>> {
        self.spectrum.clone()
    }
}

struct Reader {
    config: VisualizerConfig,
    active: Arc<AtomicBool>,
    tx: watch::Sender<Arc<Spectrum>>,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    /// The most recent mono samples, oldest first.
    samples: Vec<f32>,
}

impl Reader {
    fn new(
        config: VisualizerConfig,
        active: Arc<AtomicBool>,
        tx: watch::Sender<Arc<Spectrum>>,
    ) -> Self {
        let window = (0..FFT_SIZE)
            .map(|i| {
                let phase = 2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();
        Self {
            fft: RealFftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE),
            config,
            active,
            tx,
            window,
            samples: vec![0.0; FFT_SIZE],
        }
    }

    fn run(mut self) {
        loop {
            while !self.active.load(Ordering::Relaxed) {
                if self.tx.is_closed() {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            match open_fifo(&self.config) {
                Ok(file) => self.read_while_active(file),
                Err(error) => {
                    self.publish(Spectrum {
                        error: Some(format!(
                            "cannot open {}: {error}",
                            self.config.fifo.display()
                        )),
                        ..Spectrum::default()
                    });
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    fn read_while_active(&mut self, mut file: File) {
        let channels = usize::from(self.config.channels);
        let frame_bytes = 2 * channels;
        let interval = Duration::from_secs_f64(1.0 / f64::from(self.config.framerate));
        let mut buf = vec![0u8; 16 * 1024];
        let mut pending: Vec<u8> = Vec::new();
        let mut last_frame = Instant::now();
        let mut last_data = Instant::now();
        let mut silent = false;

        // Drop audio that piled up while nobody was reading.
        while matches!(file.read(&mut buf), Ok(n) if n > 0) {}

        while self.active.load(Ordering::Relaxed) {
            match file.read(&mut buf) {
                Ok(0) => thread::sleep(Duration::from_millis(10)),
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    let usable = pending.len() - pending.len() % frame_bytes;
                    for frame in pending[..usable].chunks_exact(frame_bytes) {
                        let sum: f32 = frame
                            .chunks_exact(2)
                            .map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])))
                            .sum();
                        self.samples
                            .push(sum / (channels as f32 * f32::from(i16::MAX)));
                    }
                    pending.drain(..usable);
                    let excess = self.samples.len().saturating_sub(FFT_SIZE);
                    self.samples.drain(..excess);
                    last_data = Instant::now();
                    silent = false;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(4));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    warn!("reading visualizer fifo failed: {error}");
                    return;
                }
            }
            if last_frame.elapsed() >= interval {
                last_frame = Instant::now();
                if last_data.elapsed() > Duration::from_millis(150) {
                    // Paused or stopped: publish silence once so bars fall.
                    if !silent {
                        silent = true;
                        self.samples.fill(0.0);
                        self.analyze();
                    }
                } else {
                    self.analyze();
                }
            }
        }
    }

    fn analyze(&mut self) {
        let mut input: Vec<f32> = self
            .samples
            .iter()
            .zip(&self.window)
            .map(|(s, w)| s * w)
            .collect();
        let mut output = self.fft.make_output_vec();
        if self.fft.process(&mut input, &mut output).is_err() {
            return;
        }
        // A full-scale sine yields |X| = N/4 with a Hann window.
        let scale = 4.0 / FFT_SIZE as f32;
        let magnitudes = output.iter().map(|c| c.norm() * scale).collect();
        self.publish(Spectrum {
            magnitudes,
            bin_hz: self.config.sample_rate as f32 / FFT_SIZE as f32,
            error: None,
        });
    }

    fn publish(&self, spectrum: Spectrum) {
        let _ = self.tx.send(Arc::new(spectrum));
    }
}

fn open_fifo(config: &VisualizerConfig) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Never block, whether or not MPD has the fifo open for writing.
        options.custom_flags(libc::O_NONBLOCK);
    }
    options.open(&config.fifo)
}
