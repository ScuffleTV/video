use imgref::{Img, ImgVec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
	pub image: ImgVec<rgb::RGBA8>,
	pub duration_ts: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRef<'a> {
	pub image: Img<&'a [rgb::RGBA8]>,
	pub duration_ts: u64,
}

impl FrameRef<'_> {
	pub fn to_owned(self) -> Frame {
		Frame {
			image: Img::new(self.image.buf().to_vec(), self.image.width(), self.image.height()),
			duration_ts: self.duration_ts,
		}
	}
}

impl Frame {
	pub fn new(width: usize, height: usize) -> Self {
		Self {
			image: ImgVec::new(vec![rgb::RGBA8::default(); width * height], width, height),
			duration_ts: 0,
		}
	}

	pub fn as_ref(&self) -> FrameRef<'_> {
		FrameRef {
			image: self.image.as_ref(),
			duration_ts: self.duration_ts,
		}
	}
}

impl<'a> FrameRef<'a> {
	pub fn new(buf: &'a [rgb::RGBA8], width: usize, height: usize, duration_ts: u64) -> Self {
		Self {
			duration_ts,
			image: Img::new(buf, width, height),
		}
	}
}
