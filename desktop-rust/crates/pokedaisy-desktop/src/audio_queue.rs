use sdl2::audio::AudioCallback;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct QueueState {
    samples: VecDeque<i16>,
    underrun_frames: u64,
}

#[derive(Clone, Default)]
pub struct AudioPlaybackQueue {
    state: Arc<Mutex<QueueState>>,
    active: Arc<AtomicBool>,
}

pub struct AudioPlaybackCallback {
    state: Arc<Mutex<QueueState>>,
    active: Arc<AtomicBool>,
}

impl AudioPlaybackQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn callback(&self) -> AudioPlaybackCallback {
        AudioPlaybackCallback {
            state: Arc::clone(&self.state),
            active: Arc::clone(&self.active),
        }
    }

    pub fn queue(&self, samples: &[i16]) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        state.samples.extend(samples.iter().copied());
        true
    }

    pub fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.samples.clear();
        }
    }

    pub fn queued_bytes(&self) -> u32 {
        self.state
            .lock()
            .map(|state| {
                state
                    .samples
                    .len()
                    .saturating_mul(std::mem::size_of::<i16>())
                    .min(u32::MAX as usize) as u32
            })
            .unwrap_or(0)
    }

    pub fn underrun_frames(&self) -> u64 {
        self.state
            .lock()
            .map(|state| state.underrun_frames)
            .unwrap_or(0)
    }

    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }
}

impl AudioCallback for AudioPlaybackCallback {
    type Channel = i16;

    fn callback(&mut self, output: &mut [i16]) {
        output.fill(0);
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let available = output.len().min(state.samples.len());
        for sample in &mut output[..available] {
            *sample = state.samples.pop_front().unwrap_or_default();
        }
        if available < output.len() && self.active.load(Ordering::Relaxed) {
            let missing_frames = (output.len() - available).div_ceil(2) as u64;
            state.underrun_frames = state.underrun_frames.saturating_add(missing_frames);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AudioPlaybackQueue;
    use sdl2::audio::AudioCallback;

    #[test]
    fn callback_drains_stereo_samples_and_counts_missing_device_frames() {
        let queue = AudioPlaybackQueue::new();
        queue.set_active(true);
        assert!(queue.queue(&[100, -100]));
        let mut callback = queue.callback();
        let mut output = [7_i16; 4];

        callback.callback(&mut output);

        assert_eq!(output, [100, -100, 0, 0]);
        assert_eq!(queue.underrun_frames(), 1);
        assert_eq!(queue.queued_bytes(), 0);
    }

    #[test]
    fn callback_does_not_report_underruns_while_emulation_is_inactive() {
        let queue = AudioPlaybackQueue::new();
        let mut callback = queue.callback();
        let mut output = [1_i16; 8];

        callback.callback(&mut output);

        assert_eq!(output, [0; 8]);
        assert_eq!(queue.underrun_frames(), 0);
    }

    #[test]
    fn queued_samples_are_stereo_byte_counted_and_clearable() {
        let queue = AudioPlaybackQueue::new();
        assert!(queue.queue(&[1, 2, 3, 4]));
        assert_eq!(queue.queued_bytes(), 8);
        queue.clear();
        assert_eq!(queue.queued_bytes(), 0);
    }
}
