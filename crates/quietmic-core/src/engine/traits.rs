/// Standard Trait for Real-Time Denoising Engines.
/// Every engine operates on 10 ms frames (480 samples at 48 kHz).
/// Must be free of heap allocations in `process_frame`.
pub trait DenoiseEngine: Send {
    /// Name of the engine (e.g. "Light (RNNoise)", "Standard (DeepFilterNet)").
    fn name(&self) -> &'static str;

    /// Resets any recurrent hidden state (e.g. GRUs, STFT history).
    fn reset(&mut self);

    /// Processes a single 10 ms mono frame (480 samples at 48 kHz).
    ///
    /// # Arguments
    /// * `in_frame` - 480 input samples in [-1.0, 1.0].
    /// * `out_frame` - 480 preallocated output samples.
    fn process_frame(&mut self, in_frame: &[f32], out_frame: &mut [f32]) -> Result<(), String>;
}
