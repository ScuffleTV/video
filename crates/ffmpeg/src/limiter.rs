use ffmpeg_sys_next::AVRational;

use crate::frame::Frame;

#[derive(Debug)]
pub struct FrameRateLimiter {
	last_frame: i64,
	accumulated_time: i64,
	frame_timing: i64,
}

impl FrameRateLimiter {
	pub fn new(frame_rate: i32, time_base: AVRational) -> Self {
		let frame_timing = ((time_base.den / frame_rate) / time_base.num) as i64;
		Self {
			last_frame: 0,
			accumulated_time: 0,
			frame_timing,
		}
	}

	pub fn limit(&mut self, frame: &Frame) -> bool {
		let ts = frame.dts().unwrap_or_else(|| frame.pts().unwrap());
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
