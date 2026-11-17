use std::time::Duration;

const FALLBACK_FPS: f64 = 60.0;

pub fn frame_interval(fps: f64) -> Duration {
    let fps = if fps.is_finite() && fps > 0.0 {
        fps.clamp(1.0, 1000.0)
    } else {
        FALLBACK_FPS
    };
    Duration::from_secs_f64(1.0 / fps)
}

pub fn next_frame_anchor(
    previous: std::time::Instant,
    now: std::time::Instant,
    interval: Duration,
) -> std::time::Instant {
    let next = previous + interval;
    if now.saturating_duration_since(next) > interval.saturating_mul(2) {
        now
    } else {
        next
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioQueueSnapshot {
    pub depth_ms: u32,
    pub high_water_ms: u32,
    pub empty_episodes: u64,
    pub dropped_frames: u64,
}

#[derive(Default)]
pub struct AudioQueueDiagnostics {
    snapshot: AudioQueueSnapshot,
    was_empty: bool,
}

impl AudioQueueDiagnostics {
    pub fn observe(&mut self, queued_bytes: u32, sample_rate: u32, dropped_frames: u64) {
        let bytes_per_second = u64::from(sample_rate).saturating_mul(4);
        let depth_ms = if bytes_per_second == 0 {
            0
        } else {
            (u64::from(queued_bytes).saturating_mul(1000) / bytes_per_second)
                .min(u64::from(u32::MAX)) as u32
        };
        self.snapshot.depth_ms = depth_ms;
        self.snapshot.high_water_ms = self.snapshot.high_water_ms.max(depth_ms);
        if depth_ms == 0 && !self.was_empty {
            self.snapshot.empty_episodes = self.snapshot.empty_episodes.saturating_add(1);
        }
        self.was_empty = depth_ms == 0;
        self.snapshot.dropped_frames = self.snapshot.dropped_frames.saturating_add(dropped_frames);
    }

    pub fn snapshot(&self) -> AudioQueueSnapshot {
        self.snapshot
    }
}

#[derive(Default)]
pub struct StereoResampler {
    rates: Option<(f64, u32)>,
    phase: f64,
    carry: Vec<i16>,
}

impl StereoResampler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.rates = None;
        self.phase = 0.0;
        self.carry.clear();
    }

    pub fn process(&mut self, input: &[i16], source_rate: f64, target_rate: u32) -> Vec<i16> {
        if input.is_empty() {
            return Vec::new();
        }
        if !source_rate.is_finite() || source_rate <= 0.0 || target_rate == 0 {
            self.reset();
            return input.to_vec();
        }
        let input_frames = input.len() / 2;
        if input_frames == 0 {
            return Vec::new();
        }
        if self.rates != Some((source_rate, target_rate)) {
            self.reset();
            self.rates = Some((source_rate, target_rate));
        }
        self.carry.extend_from_slice(&input[..input_frames * 2]);
        let buffered_frames = self.carry.len() / 2;
        let step = source_rate / target_rate as f64;
        let mut output = Vec::new();
        while self.phase + 1.0 < buffered_frames as f64 {
            let first = self.phase.floor() as usize;
            let second = first + 1;
            let fraction = self.phase - first as f64;
            for channel in 0..2 {
                let a = self.carry[first * 2 + channel] as f64;
                let b = self.carry[second * 2 + channel] as f64;
                output.push(
                    (a + (b - a) * fraction)
                        .round()
                        .clamp(i16::MIN as f64, i16::MAX as f64) as i16,
                );
            }
            self.phase += step;
        }
        let consumed_frames = (self.phase.floor() as usize).min(buffered_frames.saturating_sub(1));
        if consumed_frames > 0 {
            self.carry.drain(..consumed_frames * 2);
            self.phase -= consumed_frames as f64;
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioQueueDiagnostics, StereoResampler, frame_interval, next_frame_anchor};
    use std::time::Duration;

    #[test]
    fn frame_interval_uses_core_fps() {
        assert_eq!(frame_interval(50.0), Duration::from_secs_f64(1.0 / 50.0));
        assert_eq!(frame_interval(59.94), Duration::from_secs_f64(1.0 / 59.94));
        assert_eq!(frame_interval(60.0), Duration::from_secs_f64(1.0 / 60.0));
    }

    #[test]
    fn frame_interval_falls_back_for_invalid_core_timing() {
        assert_eq!(
            frame_interval(f64::NAN),
            Duration::from_secs_f64(1.0 / 60.0)
        );
        assert_eq!(frame_interval(0.0), Duration::from_secs_f64(1.0 / 60.0));
    }

    #[test]
    fn frame_anchor_advances_without_small_execution_time_drift() {
        let start = std::time::Instant::now();
        let interval = Duration::from_millis(20);
        assert_eq!(
            next_frame_anchor(start, start + interval + Duration::from_millis(2), interval),
            start + interval
        );
    }

    #[test]
    fn frame_anchor_drops_backlog_after_a_long_stall() {
        let start = std::time::Instant::now();
        let interval = Duration::from_millis(20);
        let now = start + Duration::from_millis(100);
        assert_eq!(next_frame_anchor(start, now, interval), now);
    }

    #[test]
    fn streaming_resampler_interpolates_across_frame_boundaries() {
        let mut resampler = StereoResampler::new();
        let first = resampler.process(&[0, 0, 10, 10], 22_050.0, 44_100);
        let second = resampler.process(&[20, 20], 22_050.0, 44_100);
        assert_eq!(first, [0, 0, 5, 5]);
        assert_eq!(second, [10, 10, 15, 15]);
    }

    #[test]
    fn resampler_handles_empty_and_invalid_rates() {
        let mut resampler = StereoResampler::new();
        assert!(resampler.process(&[], 22_050.0, 44_100).is_empty());
        let input = [7, -7];
        assert_eq!(resampler.process(&input, 0.0, 44_100), input);
    }

    #[test]
    fn streaming_resampler_carries_fractional_phase_across_frames() {
        let mut resampler = StereoResampler::new();
        let frame = [1200, -1200].repeat(735);
        let mut output_frames = 0usize;
        for _ in 0..60 {
            output_frames += resampler.process(&frame, 44_100.0, 32_768).len() / 2;
        }
        assert!(
            output_frames.abs_diff(32_768) <= 1,
            "60 emulated frames at 60 Hz should resample to about 32,768 Hz, got {output_frames}"
        );
    }

    #[test]
    fn audio_queue_diagnostics_track_depth_empty_episodes_and_dropped_frames() {
        let mut diagnostics = AudioQueueDiagnostics::default();
        diagnostics.observe(0, 44_100, 0);
        diagnostics.observe(17_640, 44_100, 12);
        diagnostics.observe(0, 44_100, 0);
        diagnostics.observe(0, 44_100, 0);

        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.depth_ms, 0);
        assert_eq!(snapshot.high_water_ms, 100);
        assert_eq!(snapshot.empty_episodes, 2);
        assert_eq!(snapshot.dropped_frames, 12);
    }
}
