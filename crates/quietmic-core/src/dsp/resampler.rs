use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

/// Ultra-low latency audio resampler wrapping rubato.
/// Provides sample-rate conversion between arbitrary hardware rates (e.g. 16 kHz Bluetooth HFP,
/// 44.1 kHz, 48 kHz, 96 kHz) and the internal standard 48 kHz processing pipeline.
pub struct AudioResampler {
    resampler: Option<SincFixedIn<f32>>,
    in_rate: u32,
    out_rate: u32,
    input_buffer: Vec<Vec<f32>>,
    pending: Vec<f32>,
}

impl AudioResampler {
    /// Creates a new AudioResampler.
    /// If `in_rate == out_rate`, resampling is bypassed completely for zero CPU cost.
    pub fn new(in_rate: u32, out_rate: u32, chunk_size: usize) -> Result<Self, String> {
        if in_rate == out_rate {
            return Ok(Self {
                resampler: None,
                in_rate,
                out_rate,
                input_buffer: Vec::new(),
                pending: Vec::new(),
            });
        }

        // Fast low-latency sinc interpolation parameters
        let params = SincInterpolationParameters {
            sinc_len: 64,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };

        let resampler = SincFixedIn::<f32>::new(
            out_rate as f64 / in_rate as f64,
            2.0,
            params,
            chunk_size,
            1, // Mono
        ).map_err(|e| format!("Failed to initialize rubato resampler: {}", e))?;

        let input_buffer = vec![vec![0.0f32; chunk_size]];
        let pending = Vec::with_capacity(chunk_size * 4);

        Ok(Self {
            resampler: Some(resampler),
            in_rate,
            out_rate,
            input_buffer,
            pending,
        })
    }

    /// Resamples a mono input slice into the destination vector without zero-padding discontinuities.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<(), String> {
        output.clear();

        if self.resampler.is_none() {
            // Bypass mode: identical sample rates
            output.extend_from_slice(input);
            return Ok(());
        }

        let resampler = self.resampler.as_mut().unwrap();
        let chunk_size = self.input_buffer[0].len();

        // Append input into pre-allocated FIFO so 479/481-sample drift frames never pad with zeros
        if self.pending.len() + input.len() > self.pending.capacity() {
            let keep = chunk_size.min(self.pending.len());
            let drop_n = self.pending.len() - keep;
            if drop_n > 0 {
                self.pending.copy_within(drop_n.., 0);
                self.pending.truncate(keep);
            }
        }
        self.pending.extend_from_slice(input);

        while self.pending.len() >= chunk_size {
            self.input_buffer[0].copy_from_slice(&self.pending[..chunk_size]);
            let rem = self.pending.len() - chunk_size;
            if rem > 0 {
                self.pending.copy_within(chunk_size.., 0);
            }
            self.pending.truncate(rem);

            let resampled = resampler.process(&self.input_buffer, None)
                .map_err(|e| format!("Resampling error: {}", e))?;

            if let Some(ch) = resampled.first() {
                output.extend_from_slice(ch);
            }
        }

        Ok(())
    }

    #[inline]
    pub fn in_rate(&self) -> u32 {
        self.in_rate
    }

    #[inline]
    pub fn out_rate(&self) -> u32 {
        self.out_rate
    }
}
