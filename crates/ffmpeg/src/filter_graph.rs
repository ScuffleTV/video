use std::ffi::CString;
use std::ptr::NonNull;

use ffmpeg_sys_next::*;

use crate::error::{FfmpegError, AVERROR_EAGAIN};
use crate::frame::Frame;
use crate::smart_object::SmartPtr;

pub struct FilterGraph(SmartPtr<AVFilterGraph>);

/// Safety: `FilterGraph` is safe to send between threads.
unsafe impl Send for FilterGraph {}

impl FilterGraph {
	pub fn new() -> Result<Self, FfmpegError> {
		// Safety: the pointer returned from avfilter_graph_alloc is valid
		unsafe { Self::wrap(avfilter_graph_alloc()) }
	}

	/// Safety: `ptr` must be a valid pointer to an `AVFilterGraph`.
	unsafe fn wrap(ptr: *mut AVFilterGraph) -> Result<Self, FfmpegError> {
		Ok(Self(
			SmartPtr::wrap_non_null(ptr, |ptr| unsafe { avfilter_graph_free(ptr) }).ok_or(FfmpegError::Alloc)?,
		))
	}

	pub fn as_ptr(&self) -> *const AVFilterGraph {
		self.0.as_ptr()
	}

	pub fn as_mut_ptr(&mut self) -> *mut AVFilterGraph {
		self.0.as_mut_ptr()
	}

	pub fn add(&mut self, filter: Filter, name: &str, args: &str) -> Result<FilterContext<'_>, FfmpegError> {
		let name = CString::new(name).expect("failed to convert name to CString");
		let args = CString::new(args).expect("failed to convert args to CString");

		let mut filter_context = std::ptr::null_mut();

		// Safety: avfilter_graph_create_filter is safe to call, 'filter_context' is a
		// valid pointer
		let ret = unsafe {
			avfilter_graph_create_filter(
				&mut filter_context,
				filter.as_ptr(),
				name.as_ptr(),
				args.as_ptr(),
				std::ptr::null_mut(),
				self.as_mut_ptr(),
			)
		};

		if ret < 0 {
			Err(FfmpegError::Code(ret.into()))
		} else {
			// Safety: 'filter_context' is a valid pointer
			Ok(FilterContext(unsafe {
				NonNull::new(filter_context).ok_or(FfmpegError::Alloc)?.as_mut()
			}))
		}
	}

	pub fn get(&mut self, name: &str) -> Option<FilterContext<'_>> {
		let name = CString::new(name).unwrap();
		// Safety: avfilter_graph_get_filter is safe to call, and the returned pointer
		// is valid
		let mut ptr = NonNull::new(unsafe { avfilter_graph_get_filter(self.as_mut_ptr(), name.as_ptr()) })?;
		Some(FilterContext(unsafe { ptr.as_mut() }))
	}

	pub fn validate(&mut self) -> Result<(), FfmpegError> {
		// Safety: avfilter_graph_config is safe to call
		let ret = unsafe { avfilter_graph_config(self.as_mut_ptr(), std::ptr::null_mut()) };

		if ret < 0 {
			Err(FfmpegError::Code(ret.into()))
		} else {
			Ok(())
		}
	}

	pub fn dump(&mut self) -> Option<String> {
		unsafe {
			// Safety: avfilter_graph_dump is safe to call, and the returned pointer is
			// valid
			let c_str = SmartPtr::wrap_non_null(avfilter_graph_dump(self.as_mut_ptr(), std::ptr::null_mut()), |ptr| {
				av_free(*ptr as *mut libc::c_void);
				*ptr = std::ptr::null_mut();
			})?;

			// Safety: the lifetime of c_str does not exceed the lifetime of the the `CStr`
			// returned by `from_ptr`
			let c_str = std::ffi::CStr::from_ptr(c_str.as_ptr());
			Some(c_str.to_str().ok()?.to_owned())
		}
	}

	pub fn set_thread_count(&mut self, threads: i32) {
		self.0.as_deref_mut_except().nb_threads = threads;
	}

	pub fn input(&mut self, name: &str, pad: i32) -> Result<FilterGraphParser<'_>, FfmpegError> {
		FilterGraphParser::new(self).input(name, pad)
	}

	pub fn output(&mut self, name: &str, pad: i32) -> Result<FilterGraphParser<'_>, FfmpegError> {
		FilterGraphParser::new(self).output(name, pad)
	}
}

pub struct FilterGraphParser<'a> {
	graph: &'a mut FilterGraph,
	inputs: SmartPtr<AVFilterInOut>,
	outputs: SmartPtr<AVFilterInOut>,
}

/// Safety: `FilterGraphParser` is safe to send between threads.
unsafe impl Send for FilterGraphParser<'_> {}

impl<'a> FilterGraphParser<'a> {
	fn new(graph: &'a mut FilterGraph) -> Self {
		Self {
			graph,
			// Safety: 'avfilter_inout_free' is safe to call with a null pointer, and the pointer is valid
			inputs: unsafe { SmartPtr::wrap(std::ptr::null_mut(), |ptr| avfilter_inout_free(ptr)) },
			// Safety: 'avfilter_inout_free' is safe to call with a null pointer, and the pointer is valid
			outputs: unsafe { SmartPtr::wrap(std::ptr::null_mut(), |ptr| avfilter_inout_free(ptr)) },
		}
	}

	pub fn input(self, name: &str, pad: i32) -> Result<Self, FfmpegError> {
		self.inout_impl(name, pad, false)
	}

	pub fn output(self, name: &str, pad: i32) -> Result<Self, FfmpegError> {
		self.inout_impl(name, pad, true)
	}

	pub fn parse(mut self, spec: &str) -> Result<(), FfmpegError> {
		let spec = CString::new(spec).unwrap();

		// Safety: 'avfilter_graph_parse_ptr' is safe to call and all the pointers are
		// valid.
		unsafe {
			match avfilter_graph_parse_ptr(
				self.graph.as_mut_ptr(),
				spec.as_ptr(),
				self.inputs.as_mut(),
				self.outputs.as_mut(),
				std::ptr::null_mut(),
			) {
				n if n >= 0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}
	}

	fn inout_impl(mut self, name: &str, pad: i32, output: bool) -> Result<Self, FfmpegError> {
		let context = self.graph.get(name).ok_or(FfmpegError::Arguments("unknown name"))?;

		// Safety: 'avfilter_inout_alloc' is safe to call, and the returned pointer is
		// valid
		let mut inout = unsafe { SmartPtr::wrap_non_null(avfilter_inout_alloc(), |ptr| avfilter_inout_free(ptr)) }
			.ok_or(FfmpegError::Alloc)?;

		let name = CString::new(name).unwrap();

		inout.as_deref_mut_except().name = name.into_raw();
		inout.as_deref_mut_except().filter_ctx = context.0;
		inout.as_deref_mut_except().pad_idx = pad;

		if output {
			inout.as_deref_mut_except().next = self.outputs.into_inner();
			self.outputs = inout;
		} else {
			inout.as_deref_mut_except().next = self.inputs.into_inner();
			self.inputs = inout;
		}

		Ok(self)
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Filter(*const AVFilter);

impl Filter {
	pub fn get(name: &str) -> Option<Self> {
		let name = std::ffi::CString::new(name).ok()?;

		// Safety: avfilter_get_by_name is safe to call, and the returned pointer is
		// valid
		let filter = unsafe { avfilter_get_by_name(name.as_ptr()) };

		if filter.is_null() {
			None
		} else {
			Some(Self(filter))
		}
	}

	pub fn as_ptr(&self) -> *const AVFilter {
		self.0
	}

	/// # Safety
	/// `ptr` must be a valid pointer.
	pub unsafe fn wrap(ptr: *const AVFilter) -> Self {
		Self(ptr)
	}
}

/// Safety: `Filter` is safe to send between threads.
unsafe impl Send for Filter {}

pub struct FilterContext<'a>(&'a mut AVFilterContext);

/// Safety: `FilterContext` is safe to send between threads.
unsafe impl Send for FilterContext<'_> {}

impl<'a> FilterContext<'a> {
	pub fn source(self) -> FilterContextSource<'a> {
		FilterContextSource(self.0)
	}

	pub fn sink(self) -> FilterContextSink<'a> {
		FilterContextSink(self.0)
	}
}

pub struct FilterContextSource<'a>(&'a mut AVFilterContext);

/// Safety: `FilterContextSource` is safe to send between threads.
unsafe impl Send for FilterContextSource<'_> {}

impl FilterContextSource<'_> {
	pub fn send_frame(&mut self, frame: &Frame) -> Result<(), FfmpegError> {
		// Safety: `frame` is a valid pointer, and `self.0` is a valid pointer.
		unsafe {
			match av_buffersrc_write_frame(self.0, frame.as_ptr()) {
				0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}
	}

	pub fn send_eof(&mut self, pts: Option<i64>) -> Result<(), FfmpegError> {
		// Safety: `self.0` is a valid pointer.
		unsafe {
			match if let Some(pts) = pts {
				av_buffersrc_close(self.0, pts, 0)
			} else {
				av_buffersrc_write_frame(self.0, std::ptr::null())
			} {
				0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}
	}
}

pub struct FilterContextSink<'a>(&'a mut AVFilterContext);

/// Safety: `FilterContextSink` is safe to send between threads.
unsafe impl Send for FilterContextSink<'_> {}

impl FilterContextSink<'_> {
	pub fn receive_frame(&mut self) -> Result<Option<Frame>, FfmpegError> {
		let mut frame = Frame::new()?;

		// Safety: `frame` is a valid pointer, and `self.0` is a valid pointer.
		unsafe {
			match av_buffersink_get_frame(self.0, frame.as_mut_ptr()) {
				0 => Ok(Some(frame)),
				AVERROR_EAGAIN | AVERROR_EOF => Ok(None),
				e => Err(FfmpegError::Code(e.into())),
			}
		}
	}
}
