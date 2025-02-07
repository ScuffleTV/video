use std::ffi::CStr;

use ffmpeg_sys_next::*;

use super::internal::{read_packet, seek, Inner, InnerOptions};
use crate::consts::{Const, DEFAULT_BUFFER_SIZE};
use crate::dict::Dictionary;
use crate::error::{FfmpegError, FfmpegErrorCode};
use crate::packet::{Packet, Packets};
use crate::smart_object::SmartObject;
use crate::stream::Streams;

/// Represents an input stream.
pub struct Input<T: Send + Sync> {
    inner: SmartObject<Inner<T>>,
}

/// Safety: `Input` is safe to send between threads.
unsafe impl<T: Send + Sync> Send for Input<T> {}

/// Represents the options for an input stream.
#[derive(Debug, Clone)]
pub struct InputOptions<I: FnMut() -> bool> {
    /// The buffer size for the input stream.
    pub buffer_size: usize,
    /// The dictionary for the input stream.
    pub dictionary: Dictionary,
    /// The interrupt callback for the input stream.
    pub interrupt_callback: Option<I>,
}

/// Default implementation for `InputOptions`.
impl Default for InputOptions<fn() -> bool> {
    fn default() -> Self {
        Self {
            buffer_size: DEFAULT_BUFFER_SIZE,
            dictionary: Dictionary::new(),
            interrupt_callback: None,
        }
    }
}

impl<T: std::io::Read + Send + Sync> Input<T> {
    /// Creates a new `Input` instance with default options.
    pub fn new(input: T) -> Result<Self, FfmpegError> {
        Self::with_options(input, &mut InputOptions::default())
    }

    /// Creates a new `Input` instance with custom options.
    pub fn with_options(input: T, options: &mut InputOptions<impl FnMut() -> bool>) -> Result<Self, FfmpegError> {
        Self::create_input(
            Inner::new(
                input,
                InnerOptions {
                    buffer_size: options.buffer_size,
                    read_fn: Some(read_packet::<T>),
                    ..Default::default()
                },
            )?,
            None,
            &mut options.dictionary,
        )
    }

    /// Creates a new `Input` instance with seekable options.
    pub fn seekable(input: T) -> Result<Self, FfmpegError>
    where
        T: std::io::Seek,
    {
        Self::seekable_with_options(input, InputOptions::default())
    }

    /// Creates a new `Input` instance with seekable options.
    pub fn seekable_with_options(input: T, mut options: InputOptions<impl FnMut() -> bool>) -> Result<Self, FfmpegError>
    where
        T: std::io::Seek,
    {
        Self::create_input(
            Inner::new(
                input,
                InnerOptions {
                    buffer_size: options.buffer_size,
                    read_fn: Some(read_packet::<T>),
                    seek_fn: Some(seek::<T>),
                    ..Default::default()
                },
            )?,
            None,
            &mut options.dictionary,
        )
    }
}

impl<T: Send + Sync> Input<T> {
    /// Returns a constant pointer to the input stream.
    pub const fn as_ptr(&self) -> *const AVFormatContext {
        self.inner.inner_ref().context.as_ptr()
    }

    /// Returns a mutable pointer to the input stream.
    pub const fn as_mut_ptr(&mut self) -> *mut AVFormatContext {
        self.inner.inner_mut().context.as_mut_ptr()
    }

    /// Returns the streams of the input stream.
    pub const fn streams(&self) -> Const<'_, Streams<'_>> {
        // Safety: See the documentation of `Streams::new`.
        // We upcast the pointer to be mut because the function signature requires it.
        // However we do not mutate the pointer as its returned as a `Const<Streams>` which
        // restricts the mutability of the streams to be const.
        unsafe { Const::new(Streams::new(self.inner.inner_ref().context.as_ptr() as *mut _)) }
    }

    /// Returns a mutable reference to the streams of the input stream.
    pub const fn streams_mut(&mut self) -> Streams<'_> {
        // Safety: See the documentation of `Streams::new`.
        unsafe { Streams::new(self.inner.inner_mut().context.as_mut_ptr()) }
    }

    /// Returns the packets of the input stream.
    pub const fn packets(&mut self) -> Packets<'_> {
        // Safety: See the documentation of `Packets::new`.
        unsafe { Packets::new(self.inner.inner_mut().context.as_mut_ptr()) }
    }

    /// Receives a packet from the input stream.
    pub fn receive_packet(&mut self) -> Result<Option<Packet>, FfmpegError> {
        self.packets().receive()
    }

    fn create_input(mut inner: Inner<T>, path: Option<&CStr>, dictionary: &mut Dictionary) -> Result<Self, FfmpegError> {
        // Safety: avformat_open_input is safe to call
        FfmpegErrorCode(unsafe {
            avformat_open_input(
                inner.context.as_mut(),
                path.map(|p| p.as_ptr()).unwrap_or(std::ptr::null()),
                std::ptr::null(),
                dictionary.as_mut_ptr_ref(),
            )
        })
        .result()?;

        if inner.context.as_ptr().is_null() {
            return Err(FfmpegError::Alloc);
        }

        let mut inner = SmartObject::new(inner, |inner| {
            // Safety: The pointer is valid. We own this resource so we need to free it
            unsafe { avformat_close_input(inner.context.as_mut()) };
        });

        // We now own the context and this is freed when the object is dropped
        inner.context.set_destructor(|_| {});

        // Safety: avformat_find_stream_info is safe to call
        FfmpegErrorCode(unsafe { avformat_find_stream_info(inner.context.as_mut_ptr(), std::ptr::null_mut()) }).result()?;

        Ok(Self { inner })
    }
}

impl Input<()> {
    /// Opens an input stream from a file path.
    pub fn open(path: &str) -> Result<Self, FfmpegError> {
        // We immediately create an input and setup the inner, before using it.
        // Safety: When we pass this inner to `create_input` with a valid path, the inner will be initialized by ffmpeg using the path.
        let inner = unsafe { Inner::empty() };

        Self::create_input(inner, Some(&std::ffi::CString::new(path).unwrap()), &mut Dictionary::new())
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::io::Cursor;

    use insta::Settings;

    use super::{FfmpegError, Input, InputOptions, DEFAULT_BUFFER_SIZE};

    fn configure_insta_filters(settings: &mut Settings) {
        settings.add_filter(r"0x0000000000000000", "[NULL_POINTER]");
        settings.add_filter(r"0x[0-9a-f]{16}", "[NON_NULL_POINTER]");
    }

    #[test]
    fn test_input_options_default() {
        let default_options = InputOptions::default();

        assert_eq!(default_options.buffer_size, DEFAULT_BUFFER_SIZE);
        assert!(default_options.dictionary.is_empty());
        assert!(default_options.interrupt_callback.is_none());
    }

    #[test]
    fn test_open_valid_file() {
        let valid_file_path = "../../assets/avc_aac_large.mp4";
        assert!(std::path::Path::new(valid_file_path).exists(), "Test file does not exist");

        let result = Input::open(valid_file_path);
        assert!(result.is_ok(), "Expected success but got error");
    }

    #[test]
    fn test_open_invalid_path() {
        let invalid_path = "invalid_file.mp4";
        let result = Input::open(invalid_path);
        assert!(result.is_err(), "Expected an error for invalid path");
        if let Err(err) = result {
            match err {
                FfmpegError::Code(_) => (),
                _ => panic!("Unexpected error type: {:?}", err),
            }
        }
    }

    #[test]
    fn test_new_with_default_options() {
        let valid_media_data: Vec<u8> = include_bytes!("../../../../assets/avc_aac_large.mp4").to_vec();
        let data = Cursor::new(valid_media_data);
        let result = Input::new(data);

        if let Err(e) = &result {
            eprintln!("Error encountered: {:?}", e);
        }

        assert!(result.is_ok(), "Expected success but got error");
    }

    #[test]
    fn test_seekable_with_valid_input() {
        let valid_media_data: Vec<u8> = include_bytes!("../../../../assets/avc_aac_large.mp4").to_vec();
        let data = Cursor::new(valid_media_data);
        let result = Input::seekable(data);

        if let Err(e) = &result {
            eprintln!("Error encountered: {:?}", e);
        }

        assert!(result.is_ok(), "Expected success but got error");
    }

    #[test]
    fn test_as_ptr() {
        let valid_file_path = "../../assets/avc_aac_large.mp4";
        let input = Input::open(valid_file_path).expect("Failed to open valid file");

        let ptr = input.as_ptr();
        assert!(!ptr.is_null(), "Expected non-null pointer");
    }

    #[test]
    fn test_as_mut_ptr() {
        let valid_file_path = "../../assets/avc_aac_large.mp4";
        let mut input = Input::open(valid_file_path).expect("Failed to open valid file");

        let ptr = input.as_mut_ptr();
        assert!(!ptr.is_null(), "Expected non-null mutable pointer");
    }

    #[test]
    fn test_streams() {
        let valid_file_path = "../../assets/avc_aac_large.mp4";
        let input = Input::open(valid_file_path).expect("Failed to open valid file");
        let streams = input.streams();

        assert!(!streams.is_empty(), "Expected at least one stream");

        let mut settings = Settings::new();
        configure_insta_filters(&mut settings);

        settings.bind(|| {
            insta::assert_debug_snapshot!(streams, @r#"
            Streams {
                input: [NON_NULL_POINTER],
                streams: [
                    Stream {
                        index: 0,
                        id: 1,
                        time_base: AVRational {
                            num: 1,
                            den: 15360,
                        },
                        start_time: Some(
                            0,
                        ),
                        duration: Some(
                            16384,
                        ),
                        nb_frames: Some(
                            64,
                        ),
                        disposition: 1,
                        discard: AVDISCARD_DEFAULT,
                        sample_aspect_ratio: AVRational {
                            num: 1,
                            den: 1,
                        },
                        metadata: {
                            "language": "und",
                            "handler_name": "GPAC ISO Video Handler",
                            "vendor_id": "[0][0][0][0]",
                            "encoder": "Lavc60.9.100 libx264",
                        },
                        avg_frame_rate: AVRational {
                            num: 60,
                            den: 1,
                        },
                        r_frame_rate: AVRational {
                            num: 60,
                            den: 1,
                        },
                    },
                    Stream {
                        index: 1,
                        id: 2,
                        time_base: AVRational {
                            num: 1,
                            den: 48000,
                        },
                        start_time: Some(
                            0,
                        ),
                        duration: Some(
                            48096,
                        ),
                        nb_frames: Some(
                            48,
                        ),
                        disposition: 1,
                        discard: AVDISCARD_DEFAULT,
                        sample_aspect_ratio: AVRational {
                            num: 0,
                            den: 1,
                        },
                        metadata: {
                            "language": "und",
                            "handler_name": "GPAC ISO Audio Handler",
                            "vendor_id": "[0][0][0][0]",
                        },
                        avg_frame_rate: AVRational {
                            num: 0,
                            den: 0,
                        },
                        r_frame_rate: AVRational {
                            num: 0,
                            den: 0,
                        },
                    },
                ],
            }
            "#);
        });
    }

    #[test]
    fn test_packets() {
        let valid_file_path = "../../assets/avc_aac_large.mp4";
        let mut input = Input::open(valid_file_path).expect("Failed to open valid file");
        let mut packets = input.packets();

        for _ in 0..5 {
            match packets.next() {
                Some(Ok(_)) => (),
                Some(Err(e)) => panic!("Error encountered while reading packets: {:?}", e),
                None => break,
            }
        }

        let mut settings = insta::Settings::new();
        configure_insta_filters(&mut settings);

        settings.bind(|| {
            insta::assert_debug_snapshot!(packets, @r"
            Packets {
                context: [NON_NULL_POINTER],
            }
            ");
        });
    }

    #[test]
    fn test_receive_packet() {
        let valid_file_path = "../../assets/avc_aac_large.mp4";
        let mut input = Input::open(valid_file_path).expect("Failed to open valid file");

        let mut packets = Vec::new();
        while let Ok(Some(packet)) = input.receive_packet() {
            assert!(!packet.data().is_empty(), "Expected a non-empty packet");
            assert!(packet.stream_index() >= 0, "Expected a valid stream index");
            packets.push(packet);
        }

        if packets.is_empty() {
            panic!("Expected at least one packet but received none");
        }

        insta::assert_debug_snapshot!(packets);
    }
}
