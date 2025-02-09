use std::ffi::CString;
use std::ptr::NonNull;

use ffmpeg_sys_next::*;

use super::internal::{seek, write_packet, Inner, InnerOptions};
use crate::consts::DEFAULT_BUFFER_SIZE;
use crate::dict::Dictionary;
use crate::error::{FfmpegError, FfmpegErrorCode};
use crate::packet::Packet;
use crate::stream::Stream;

/// A struct that represents the options for the output.
#[derive(Debug, Clone, bon::Builder)]
pub struct OutputOptions {
    /// The buffer size for the output.
    #[builder(default = DEFAULT_BUFFER_SIZE)]
    buffer_size: usize,
    #[builder(setters(vis = "", name = format_ffi_internal))]
    format_ffi: *const AVOutputFormat,
}

impl<S: output_options_builder::State> OutputOptionsBuilder<S> {
    /// Sets the format FFI.
    ///
    /// Returns an error if the format FFI is null.
    pub fn format_ffi(
        self,
        format_ffi: *const AVOutputFormat,
    ) -> Result<OutputOptionsBuilder<output_options_builder::SetFormatFfi<S>>, FfmpegError>
    where
        S::FormatFfi: output_options_builder::IsUnset,
    {
        if format_ffi.is_null() {
            return Err(FfmpegError::Arguments("could not determine output format"));
        }

        Ok(self.format_ffi_internal(format_ffi))
    }

    /// Gets the format ffi from the format name.
    ///
    /// Returns an error if the format name is empty or the format was not found.
    #[inline]
    pub fn format_name(
        self,
        format_name: &str,
    ) -> Result<OutputOptionsBuilder<output_options_builder::SetFormatFfi<S>>, FfmpegError>
    where
        S::FormatFfi: output_options_builder::IsUnset,
    {
        self.format_name_mime_type(format_name, "")
    }

    /// Gets the format ffi from the format mime type.
    ///
    /// Returns an error if the format mime type is empty or the format was not found.
    #[inline]
    pub fn format_mime_type(
        self,
        format_mime_type: &str,
    ) -> Result<OutputOptionsBuilder<output_options_builder::SetFormatFfi<S>>, FfmpegError>
    where
        S::FormatFfi: output_options_builder::IsUnset,
    {
        self.format_name_mime_type("", format_mime_type)
    }

    /// Sets the format ffi from the format name and mime type.
    ///
    /// Returns an error if both the format name and mime type are empty or the format was not found.
    pub fn format_name_mime_type(
        self,
        format_name: &str,
        format_mime_type: &str,
    ) -> Result<OutputOptionsBuilder<output_options_builder::SetFormatFfi<S>>, FfmpegError>
    where
        S::FormatFfi: output_options_builder::IsUnset,
    {
        let c_format_name = CString::new(format_name).ok();
        let c_format_mime_type = CString::new(format_mime_type).ok();
        let c_format_name_ptr = c_format_name.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
        let c_format_mime_type_ptr = c_format_mime_type.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
        // Safety: av_guess_format is safe to call and all the arguments are valid
        let format_ffi = unsafe { av_guess_format(c_format_name_ptr, std::ptr::null(), c_format_mime_type_ptr) };
        self.format_ffi(format_ffi)
    }
}

/// A struct that represents the output.
pub struct Output<T: Send + Sync> {
    inner: Inner<T>,
    state: OutputState,
}

/// Safety: `T` must be `Send` and `Sync`.
unsafe impl<T: Send + Sync> Send for Output<T> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputState {
    Uninitialized,
    HeaderWritten,
    TrailerWritten,
}

impl<T: Send + Sync> Output<T> {
    /// Consumes the `Output` and returns the inner data.
    pub fn into_inner(mut self) -> T {
        *(self.inner.data.take().unwrap())
    }
}

impl<T: std::io::Write + Send + Sync> Output<T> {
    /// Creates a new `Output` with the given output and options.
    pub fn new(output: T, options: OutputOptions) -> Result<Self, FfmpegError> {
        Ok(Self {
            inner: Inner::new(
                output,
                InnerOptions {
                    buffer_size: options.buffer_size,
                    write_fn: Some(write_packet::<T>),
                    output_format: options.format_ffi,
                    ..Default::default()
                },
            )?,
            state: OutputState::Uninitialized,
        })
    }

    /// Creates a new `Output` with the given output and options. The output must be seekable.
    pub fn seekable(output: T, options: OutputOptions) -> Result<Self, FfmpegError>
    where
        T: std::io::Seek,
    {
        Ok(Self {
            inner: Inner::new(
                output,
                InnerOptions {
                    buffer_size: options.buffer_size,
                    write_fn: Some(write_packet::<T>),
                    seek_fn: Some(seek::<T>),
                    output_format: options.format_ffi,
                    ..Default::default()
                },
            )?,
            state: OutputState::Uninitialized,
        })
    }
}

impl<T: Send + Sync> Output<T> {
    /// Sets the metadata for the output.
    pub fn set_metadata(&mut self, metadata: Dictionary) {
        // Safety: We want to replace the metadata from the context (if one exists). This is safe as the metadata should be a valid pointer.
        unsafe {
            Dictionary::from_ptr_owned(self.inner.context.as_deref_mut_except().metadata);
        };

        self.inner.context.as_deref_mut_except().metadata = metadata.leak();
    }

    /// Returns the pointer to the underlying AVFormatContext.
    pub const fn as_ptr(&self) -> *const AVFormatContext {
        self.inner.context.as_ptr()
    }

    /// Returns the pointer to the underlying AVFormatContext.
    pub const fn as_mut_ptr(&mut self) -> *mut AVFormatContext {
        self.inner.context.as_mut_ptr()
    }

    /// Adds a new stream to the output.
    pub fn add_stream(&mut self, codec: Option<*const AVCodec>) -> Option<Stream<'_>> {
        let mut stream =
            // Safety: `avformat_new_stream` is safe to call.
            NonNull::new(unsafe { avformat_new_stream(self.as_mut_ptr(), codec.unwrap_or_else(std::ptr::null)) })?;

        // Safety: The stream is a valid non-null pointer.
        let stream = unsafe { stream.as_mut() };
        stream.id = self.inner.context.as_deref_except().nb_streams as i32 - 1;

        Some(Stream::new(stream, self.inner.context.as_mut_ptr()))
    }

    /// Copies a stream from the input to the output.
    pub fn copy_stream<'a>(&'a mut self, stream: &Stream<'_>) -> Result<Option<Stream<'a>>, FfmpegError> {
        let Some(codec_param) = stream.codec_parameters() else {
            return Ok(None);
        };

        // Safety: `avformat_new_stream` is safe to call.
        let Some(mut out_stream) = NonNull::new(unsafe { avformat_new_stream(self.as_mut_ptr(), std::ptr::null()) }) else {
            return Ok(None);
        };

        // Safety: The stream is a valid non-null pointer.
        let out_stream = unsafe { out_stream.as_mut() };

        // Safety: `avcodec_parameters_copy` is safe to call when all arguments are valid.
        FfmpegErrorCode(unsafe { avcodec_parameters_copy(out_stream.codecpar, codec_param) }).result()?;

        out_stream.id = self.inner.context.as_deref_except().nb_streams as i32 - 1;

        let mut out_stream = Stream::new(out_stream, self.inner.context.as_mut_ptr());
        out_stream.set_time_base(stream.time_base());
        out_stream.set_start_time(stream.start_time());
        out_stream.set_duration(stream.duration());

        Ok(Some(out_stream))
    }

    /// Writes the header to the output.
    pub fn write_header(&mut self) -> Result<(), FfmpegError> {
        if self.state != OutputState::Uninitialized {
            return Err(FfmpegError::Arguments("header already written"));
        }

        // Safety: `avformat_write_header` is safe to call, if the header has not been
        // written yet.
        FfmpegErrorCode(unsafe { avformat_write_header(self.as_mut_ptr(), std::ptr::null_mut()) }).result()?;
        self.state = OutputState::HeaderWritten;

        Ok(())
    }

    /// Writes the header to the output with the given options.
    pub fn write_header_with_options(&mut self, options: &mut Dictionary) -> Result<(), FfmpegError> {
        if self.state != OutputState::Uninitialized {
            return Err(FfmpegError::Arguments("header already written"));
        }

        // Safety: `avformat_write_header` is safe to call, if the header has not been
        // written yet.
        FfmpegErrorCode(unsafe { avformat_write_header(self.as_mut_ptr(), options.as_mut_ptr_ref()) }).result()?;
        self.state = OutputState::HeaderWritten;

        Ok(())
    }

    /// Writes the trailer to the output.
    pub fn write_trailer(&mut self) -> Result<(), FfmpegError> {
        if self.state != OutputState::HeaderWritten {
            return Err(FfmpegError::Arguments(
                "cannot write trailer before header or after trailer has been written",
            ));
        }

        // Safety: `av_write_trailer` is safe to call, once the header has been written.
        FfmpegErrorCode(unsafe { av_write_trailer(self.as_mut_ptr()) }).result()?;
        self.state = OutputState::TrailerWritten;

        Ok(())
    }

    /// Writes the interleaved packet to the output.
    /// The difference between this and `write_packet` is that this function
    /// writes the packet to the output and reorders the packets based on the
    /// dts and pts.
    pub fn write_interleaved_packet(&mut self, mut packet: Packet) -> Result<(), FfmpegError> {
        if self.state != OutputState::HeaderWritten {
            return Err(FfmpegError::Arguments(
                "cannot write interleaved packet before header or after trailer has been written",
            ));
        }

        // Safety: `av_interleaved_write_frame` is safe to call, once the header has
        // been written.
        FfmpegErrorCode(unsafe { av_interleaved_write_frame(self.as_mut_ptr(), packet.as_mut_ptr()) }).result()?;
        Ok(())
    }

    /// Writes the packet to the output. Without reordering the packets.
    pub fn write_packet(&mut self, packet: &Packet) -> Result<(), FfmpegError> {
        if self.state != OutputState::HeaderWritten {
            return Err(FfmpegError::Arguments(
                "cannot write packet before header or after trailer has been written",
            ));
        }

        // Safety: `av_write_frame` is safe to call, once the header has been written.
        FfmpegErrorCode(unsafe { av_write_frame(self.as_mut_ptr(), packet.as_ptr() as *mut _) }).result()?;
        Ok(())
    }

    /// Returns the flags for the output.
    pub const fn flags(&self) -> i32 {
        self.inner.context.as_deref_except().flags
    }
}

impl Output<()> {
    /// Opens the output with the given path.
    pub fn open(path: &str) -> Result<Self, FfmpegError> {
        Ok(Self {
            inner: Inner::open_output(path)?,
            state: OutputState::Uninitialized,
        })
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::ffi::CString;
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use std::ptr;

    use bytes::{Buf, Bytes};
    use ffmpeg_sys_next::AVMediaType;
    use sha2::Digest;
    use tempfile::Builder;

    use crate::dict::Dictionary;
    use crate::error::FfmpegError;
    use crate::io::output::{AVCodec, AVRational, OutputState, AVFMT_FLAG_AUTO_BSF};
    use crate::io::{Input, Output, OutputOptions};

    #[test]
    fn test_output_options_get_format_ffi_null() {
        let format_name = CString::new("mp4").unwrap();
        let format_mime_type = CString::new("").unwrap();
        // Safety: `av_guess_format` is safe to call and all arguments are valid.
        let format_ptr =
            unsafe { ffmpeg_sys_next::av_guess_format(format_name.as_ptr(), ptr::null(), format_mime_type.as_ptr()) };

        assert!(
            !format_ptr.is_null(),
            "Failed to retrieve AVOutputFormat for the given format name"
        );

        let output_options = OutputOptions::builder().format_name("mp4").unwrap().build();
        assert_eq!(output_options.format_ffi, format_ptr);
    }

    #[test]
    fn test_output_options_get_format_ffi_output_format_error() {
        match OutputOptions::builder().format_name("unknown_format") {
            Ok(_) => panic!("Expected error, got Ok"),
            Err(e) => {
                assert_eq!(e, FfmpegError::Arguments("could not determine output format"));
            }
        }
    }

    #[test]
    fn test_output_into_inner() {
        let data = Cursor::new(Vec::with_capacity(1024));
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let output = Output::new(data, options).expect("Failed to create Output");
        let inner_data = output.into_inner();

        assert!(inner_data.get_ref().is_empty());
        let buffer = inner_data.into_inner();
        assert_eq!(buffer.capacity(), 1024);
    }

    #[test]
    fn test_output_new() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let output = Output::new(data, options);

        assert!(output.is_ok());
    }

    #[test]
    fn test_output_seekable() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let output = Output::seekable(data, options);

        assert!(output.is_ok());
    }

    #[test]
    fn test_output_set_metadata() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).unwrap();
        let metadata = Dictionary::new();
        output.set_metadata(metadata);

        assert!(!output.as_ptr().is_null());
    }

    #[test]
    fn test_output_as_mut_ptr() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).expect("Failed to create Output");
        let context_ptr = output.as_mut_ptr();

        assert!(!context_ptr.is_null(), "Expected non-null pointer from as_mut_ptr");
    }

    #[test]
    fn test_add_stream_with_valid_codec() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).expect("Failed to create Output");
        let dummy_codec: *const AVCodec = 0x1234 as *const AVCodec;
        let stream = output.add_stream(Some(dummy_codec));

        assert!(stream.is_some(), "Expected a valid Stream to be added");
    }

    #[test]
    fn test_copy_stream() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).expect("Failed to create Output");

        // create new output to prevent double mut borrow
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output_two = Output::new(data, options).expect("Failed to create Output");

        let dummy_codec: *const AVCodec = 0x1234 as *const AVCodec;
        let mut source_stream = output.add_stream(Some(dummy_codec)).expect("Failed to add source stream");

        source_stream.set_time_base(AVRational { num: 1, den: 25 });
        source_stream.set_start_time(Some(1000));
        source_stream.set_duration(Some(500));
        let copied_stream = output_two
            .copy_stream(&source_stream)
            .expect("Failed to copy the stream")
            .expect("Failed to copy the stream");

        assert_eq!(copied_stream.index(), source_stream.index(), "Stream indices should match");
        assert_eq!(copied_stream.id(), source_stream.id(), "Stream IDs should match");
        assert_eq!(
            copied_stream.time_base(),
            source_stream.time_base(),
            "Time bases should match"
        );
        assert_eq!(
            copied_stream.start_time(),
            source_stream.start_time(),
            "Start times should match"
        );
        assert_eq!(copied_stream.duration(), source_stream.duration(), "Durations should match");
        assert_eq!(copied_stream.duration(), source_stream.duration(), "Durations should match");
        assert!(!copied_stream.as_ptr().is_null(), "Copied stream pointer should not be null");
    }

    #[test]
    fn test_output_flags() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let output = Output::new(data, options).expect("Failed to create Output");
        let flags = output.flags();

        assert_eq!(flags, AVFMT_FLAG_AUTO_BSF, "Expected default flag to be AVFMT_FLAG_AUTO_BSF");
    }

    #[test]
    fn test_output_open() {
        let temp_file = Builder::new()
            .suffix(".mp4")
            .tempfile()
            .expect("Failed to create a temporary file");
        let temp_path = temp_file.path();
        let output = Output::open(temp_path.to_str().unwrap());

        assert!(output.is_ok(), "Expected Output::open to succeed");
        std::fs::remove_file(temp_path).expect("Failed to remove temporary file");
    }

    macro_rules! get_boxes {
        ($output:expr) => {{
            let binary = $output.inner.data.as_mut().unwrap().get_mut().as_slice();
            let mut cursor = Cursor::new(Bytes::copy_from_slice(binary));
            let mut boxes = Vec::new();
            while cursor.has_remaining() {
                let mut box_ = scuffle_mp4::DynBox::demux(&mut cursor).expect("Failed to demux mp4");
                if let scuffle_mp4::DynBox::Mdat(mdat) = &mut box_ {
                    mdat.data.iter_mut().for_each(|buf| {
                        let mut hash = sha2::Sha256::new();
                        hash.write_all(buf).unwrap();
                        *buf = hash.finalize().to_vec().into();
                    });
                }
                boxes.push(box_);
            }

            boxes
        }};
    }

    #[test]
    fn test_output_write_mp4() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();

        let mut output = Output::seekable(data, options).expect("Failed to create Output");
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

        let mut input = Input::seekable(std::fs::File::open(dir.join("avc_aac.mp4")).expect("Failed to open file"))
            .expect("Failed to create Input");
        let streams = input.streams();
        let best_video_stream = streams.best(AVMediaType::AVMEDIA_TYPE_VIDEO).expect("no video stream found");

        output.copy_stream(&best_video_stream).expect("Failed to copy stream");

        output.write_header().expect("Failed to write header");
        assert_eq!(output.state, OutputState::HeaderWritten, "Expected header to be written");
        assert!(output.write_header().is_err(), "Expected error when writing header twice");

        insta::assert_debug_snapshot!("test_output_write_mp4_header", get_boxes!(output));

        let best_video_stream_index = best_video_stream.index();

        while let Some(packet) = input.receive_packet().expect("Failed to receive packet") {
            if packet.stream_index() != best_video_stream_index {
                continue;
            }

            output.write_interleaved_packet(packet).expect("Failed to write packet");
        }

        insta::assert_debug_snapshot!("test_output_write_mp4_packets", get_boxes!(output));

        output.write_trailer().expect("Failed to write trailer");
        assert!(output.write_trailer().is_err(), "Expected error when writing trailer twice");
        assert_eq!(output.state, OutputState::TrailerWritten, "Expected trailer to be written");

        insta::assert_debug_snapshot!("test_output_write_mp4_trailer", get_boxes!(output));
    }

    #[test]
    fn test_output_write_mp4_fragmented() {
        let data = Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();

        let mut output = Output::seekable(data, options).expect("Failed to create Output");
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

        let mut input = Input::seekable(std::fs::File::open(dir.join("avc_aac.mp4")).expect("Failed to open file"))
            .expect("Failed to create Input");
        let streams = input.streams();
        let best_video_stream = streams.best(AVMediaType::AVMEDIA_TYPE_VIDEO).expect("no video stream found");

        output.copy_stream(&best_video_stream).expect("Failed to copy stream");

        output
            .write_header_with_options(
                &mut Dictionary::try_from_iter([("movflags", "frag_keyframe+empty_moov")])
                    .expect("Failed to create dictionary from hashmap"),
            )
            .expect("Failed to write header");
        assert_eq!(output.state, OutputState::HeaderWritten, "Expected header to be written");
        assert!(
            output
                .write_header_with_options(
                    &mut Dictionary::try_from_iter([("movflags", "frag_keyframe+empty_moov")],)
                        .expect("Failed to create dictionary from hashmap")
                )
                .is_err(),
            "Expected error when writing header twice"
        );

        insta::assert_debug_snapshot!("test_output_write_mp4_fragmented_header", get_boxes!(output));

        let best_video_stream_index = best_video_stream.index();

        while let Some(packet) = input.receive_packet().expect("Failed to receive packet") {
            if packet.stream_index() != best_video_stream_index {
                continue;
            }

            output.write_packet(&packet).expect("Failed to write packet");
        }

        insta::assert_debug_snapshot!("test_output_write_mp4_fragmented_packets", get_boxes!(output));

        output.write_trailer().expect("Failed to write trailer");
        assert_eq!(output.state, OutputState::TrailerWritten, "Expected trailer to be written");
        assert!(output.write_trailer().is_err(), "Expected error when writing trailer twice");

        insta::assert_debug_snapshot!("test_output_write_mp4_fragmented_trailer", get_boxes!(output));
    }
}
