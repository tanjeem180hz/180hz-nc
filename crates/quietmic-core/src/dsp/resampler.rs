use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

/// Ultra-low latency audio resampler wrapping rubato.
/// Provides sample-rate conversion between arbitrary hardware rates (e.g. 16 kHz Bluetooth HFP,
/// 44.1 kHz, 48 kHz, 96 kHz) and the internal standard 48 kHz processing pipeline.
pub struct AudioResampler {
    resampler: Option<SincFixedIn<f32>>,
    in_rate: u32,
    out_rate: u32,
    input_buffer: Vec<Vec<f32>>,
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

        Ok(Self {
            resampler: Some(resampler),
            in_rate,
            out_rate,
            input_buffer,
        })
    }

    /// Resamples a mono input slice into the destination vector.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<(), String> {
        output.clear();

        if self.resampler.is_none() {
            // Bypass mode: identical sample rates
            output.extend_from_slice(input);
            return Ok(());
        }

        let resampler = self.resampler.as_mut().unwrap();
        let chunk_size = self.input_buffer[0].len();

        let mut offset = 0;
        while offset < input.len() {
            let available = (input.len() - offset).min(chunk_size);
            self.input_buffer[0][..available].copy_from_slice(&input[offset..offset + available]);
            if available < chunk_size {
                self.input_buffer[0][available..].fill(0.0);
            }

            let resampled = resampler.process(&self.input_buffer, None)
                .map_err(|e| format!("Resampling error: {}", e))?;

            if let Some(ch) = resampled.first() {
                output.extend_from_slice(ch);
            }

            offset += chunk_size;
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
