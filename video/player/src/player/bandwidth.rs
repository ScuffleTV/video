use super::fetch::Metrics;

#[derive(Debug)]
pub struct Bandwidth {
	fast_ewma: f64,
	slow_ewma: f64,
	fast_alpha: f64,
	slow_alpha: f64,
}

#[inline(always)]
fn alpha(half_life: f64) -> f64 {
	if half_life != 0.0 {
		(0.5_f64.ln() / half_life).exp()
	} else {
		0.0
	}
}

const FILE_SIZE_WEIGHT: f64 = 0.1;
const DURATION_WEIGHT: f64 = 2.5;

impl Bandwidth {
	pub fn new(initial_estimate: f64) -> Self {
		Self {
			fast_ewma: initial_estimate,
			slow_ewma: initial_estimate,
			fast_alpha: alpha(3.0),
			slow_alpha: alpha(9.0),
		}
	}

	pub fn update_alpha(&mut self, fast: f64, slow: f64) {
		self.fast_alpha = alpha(fast);
		self.slow_alpha = alpha(slow);
	}

	pub fn sample(&mut self, metrics: &Metrics) {
		if metrics.size == 0 {
			return;
		}

		let ttfb = metrics.ttfb.max(metrics.relative_ttfb);

		let adjusted_duration = (metrics.total_duration - ttfb).max(2.0);

		let file_size = metrics.size as f64 * 8.0;

		let bandwidth = file_size / (adjusted_duration / 1000.0);

		let file_duration = metrics.file_duration * 1000.0;

		let duration_ratio_weighted = (adjusted_duration / file_duration) * DURATION_WEIGHT;
		let file_size_mb_weighted = (file_size / 8.0 / 1024.0 / 1024.0) * FILE_SIZE_WEIGHT;

		let weight = file_size_mb_weighted + duration_ratio_weighted;

		let alpha_fast = self.fast_alpha.powf(weight).clamp(0.8, 1.0);
		let alpha_slow = self.slow_alpha.powf(weight).clamp(0.8, 1.0);

		self.fast_ewma = bandwidth * (1.0 - alpha_fast) + self.fast_ewma * alpha_fast;
		self.slow_ewma = bandwidth * (1.0 - alpha_slow) + self.slow_ewma * alpha_slow;
	}

	pub fn estimate(&self) -> f64 {
		self.fast_ewma.min(self.slow_ewma)
	}
}
