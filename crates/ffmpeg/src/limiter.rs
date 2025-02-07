use ffmpeg_sys_next::AVRational;

use crate::frame::Frame;

/// A frame rate limiter is used to limit the frame rate of a video. By dropping frames if there are too many.
#[derive(Debug)]
pub struct FrameRateLimiter {
    last_frame: i64,
    accumulated_time: i64,
    frame_timing: i64,
}

impl FrameRateLimiter {
    /// Creates a new frame rate limiter.
    ///
    /// Returns None if any of the arguments are invalid. (frame_rate == 0 || time_base.num == 0 || time_base.den == 0)
    pub const fn new(frame_rate: i32, time_base: AVRational) -> Option<Self> {
        if frame_rate == 0 || time_base.num == 0 || time_base.den == 0 {
            return None;
        }

        Some(Self {
            last_frame: 0,
            accumulated_time: 0,
            frame_timing: ((time_base.den / frame_rate) / time_base.num) as i64,
        })
    }

    /// Limits the frame rate.
    pub const fn limit(&mut self, frame: &Frame) -> bool {
        let ts = match frame.dts() {
            Some(dts) => dts,
            None => match frame.pts() {
                Some(pts) => pts,
                None => return false,
            },
        };

        let delta = ts - self.last_frame;
        self.last_frame = ts;
        self.accumulated_time += delta;
        if self.accumulated_time >= self.frame_timing {
            self.accumulated_time -= self.frame_timing;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use ffmpeg_sys_next::AVRational;

    use crate::frame::Frame;
    use crate::limiter::FrameRateLimiter;

    #[test]
    fn test_frame_rate_limiter_new() {
        let frame_rate = 30;
        let time_base = AVRational { num: 1, den: 30000 };

        let limiter = FrameRateLimiter::new(frame_rate, time_base).unwrap();

        assert_eq!(limiter.last_frame, 0);
        assert_eq!(limiter.accumulated_time, 0);
        assert_eq!(limiter.frame_timing, 1000);
    }

    #[test]
    fn test_frame_rate_limiter_limit_if_case() {
        let frame_rate = 30;
        let time_base = AVRational { num: 1, den: 30000 };
        let mut limiter = FrameRateLimiter::new(frame_rate, time_base).unwrap();
        let mut frame = Frame::new().unwrap();
        frame.set_dts(Some(2000));
        let result = limiter.limit(&frame);

        assert!(result);
        assert_eq!(limiter.last_frame, 2000);
        assert_eq!(limiter.accumulated_time, 1000);
    }

    #[test]
    fn test_frame_rate_limiter_limit_else_case() {
        let frame_rate = 30;
        let time_base = AVRational { num: 1, den: 30000 };
        let mut limiter = FrameRateLimiter::new(frame_rate, time_base).unwrap();
        let mut frame = Frame::new().unwrap();
        frame.set_dts(Some(500));
        let result = limiter.limit(&frame);

        assert!(!result);
        assert_eq!(limiter.last_frame, 500);
        assert_eq!(limiter.accumulated_time, 500);
    }

    #[test]
    fn test_frame_rate_limiter_new_invalid_frame_rate() {
        let frame_rate = 0;
        let time_base = AVRational { num: 1, den: 30000 };
        let limiter = FrameRateLimiter::new(frame_rate, time_base);

        assert!(limiter.is_none());
    }

    #[test]
    fn test_frame_rate_limiter_new_invalid_time_base_den() {
        let frame_rate = 30;
        let time_base = AVRational { num: 1, den: 0 };
        let limiter = FrameRateLimiter::new(frame_rate, time_base);

        assert!(limiter.is_none());
    }

    #[test]
    fn test_frame_rate_limiter_new_invalid_time_base_num() {
        let frame_rate = 30;
        let time_base = AVRational { num: 0, den: 1 };
        let limiter = FrameRateLimiter::new(frame_rate, time_base);

        assert!(limiter.is_none());
    }
}
