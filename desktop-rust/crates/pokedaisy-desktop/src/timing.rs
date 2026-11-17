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

pub fn resample_stereo(input: &[i16], source_rate: f64, target_rate: u32) -> Vec<i16> {
    if input.is_empty() || !source_rate.is_finite() || source_rate <= 0.0 || target_rate == 0 {
        return input.to_vec();
    }

    let input_frames = input.len() / 2;
    if input_frames == 0 {
        return Vec::new();
    }
    let output_frames =
        ((input_frames as f64 * target_rate as f64 / source_rate).round() as usize).max(1);
    let mut output = Vec::with_capacity(output_frames.saturating_mul(2));
    for frame in 0..output_frames {
        let source_position = frame as f64 * source_rate / target_rate as f64;
        let first = (source_position.floor() as usize).min(input_frames - 1);
        let second = (first + 1).min(input_frames - 1);
        let fraction = (source_position - first as f64).clamp(0.0, 1.0);
        for channel in 0..2 {
            let a = input[first * 2 + channel] as f64;
            let b = input[second * 2 + channel] as f64;
            output.push(
                (a + (b - a) * fraction)
                    .round()
                    .clamp(i16::MIN as f64, i16::MAX as f64) as i16,
            );
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{frame_interval, resample_stereo};
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
    fn resamples_stereo_frame_count_to_device_rate() {
        let input = (0..2205).flat_map(|n| [n, -n]).collect::<Vec<i16>>();
        let output = resample_stereo(&input, 22_050.0, 44_100);
        assert_eq!(output.len(), 8820);
        assert_eq!(&output[..4], &[0, 0, 1, -1]);
    }

    #[test]
    fn resampler_handles_empty_and_invalid_rates() {
        assert!(resample_stereo(&[], 22_050.0, 44_100).is_empty());
        let input = [7, -7];
        assert_eq!(resample_stereo(&input, 0.0, 44_100), input);
    }
}
