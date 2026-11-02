#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::System::Threading::AvSetMmThreadCharacteristicsW;

/// Windows MMCSS (Multimedia Class Scheduler Service) registration handle.
/// Guarantees that the dedicated audio processing thread receives "Pro Audio" high-priority scheduling
/// to prevent dropouts, buffer starvation, or latency spikes caused by background Windows processes.
pub struct MmcssHandle {
    #[cfg(windows)]
    _task_index: u32,
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
}

impl MmcssHandle {
    /// Registers the calling thread with Windows MMCSS under the "Pro Audio" profile.
    pub fn register_pro_audio() -> Result<Self, String> {
        #[cfg(windows)]
        unsafe {
            let mut task_index = 0u32;
            let class_name: Vec<u16> = "Pro Audio\0".encode_utf16().collect();
            let handle = AvSetMmThreadCharacteristicsW(PCWSTR(class_name.as_ptr()), &mut task_index)
                .map_err(|e| format!("AvSetMmThreadCharacteristicsW failed: {}", e))?;

            tracing::info!("Registered audio processing thread with MMCSS 'Pro Audio' (task_index={})", task_index);
            Ok(Self { _task_index: task_index, handle })
        }

        #[cfg(not(windows))]
        {
            Ok(Self {})
        }
    }
}

impl Drop for MmcssHandle {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            if !self.handle.is_invalid() {
                let _ = windows::Win32::System::Threading::AvRevertMmThreadCharacteristics(self.handle);
                tracing::info!("Reverted MMCSS Pro Audio thread characteristics");
            }
        }
    }
}
