use std::marker::PhantomData;

use ffmpeg_sys_next::*;

use crate::error::{FfmpegError, FfmpegErrorCode};
use crate::smart_object::SmartPtr;
use crate::utils::{check_i64, or_nopts};

/// A collection of packets. [`Packets`] implements [`Iterator`] and will yield packets until the end of the stream is reached.
/// A wrapper around an [`AVFormatContext`].
pub struct Packets<'a> {
    context: *mut AVFormatContext,
    _marker: PhantomData<&'a mut AVFormatContext>,
}

impl std::fmt::Debug for Packets<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Packets").field("context", &self.context).finish()
    }
}

/// Safety: `Packets` is safe to send between threads.
unsafe impl Send for Packets<'_> {}

impl Packets<'_> {
    /// Creates a new `Packets` instance.
    ///
    /// # Safety
    /// This function is unsafe because the caller must ensure that the lifetime & the mutablity
    /// of the `AVFormatContext` matches the lifetime & mutability of the `Packets`.
    pub const unsafe fn new(context: *mut AVFormatContext) -> Self {
        Self {
            context,
            _marker: PhantomData,
        }
    }

    /// Receives a packet from the context.
    pub fn receive(&mut self) -> Result<Option<Packet>, FfmpegError> {
        let mut packet = Packet::new()?;

        // Safety: av_read_frame is safe to call, 'packet' is a valid pointer
        match FfmpegErrorCode(unsafe { av_read_frame(self.context, packet.as_mut_ptr()) }) {
            code if code.is_success() => Ok(Some(packet)),
            FfmpegErrorCode::Eof => Ok(None),
            code => Err(FfmpegError::Code(code)),
        }
    }
}

impl Iterator for Packets<'_> {
    type Item = Result<Packet, FfmpegError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.receive().transpose()
    }
}

/// A packet is a wrapper around an [`AVPacket`].
pub struct Packet(SmartPtr<AVPacket>);

/// Safety: `Packet` is safe to send between threads.
unsafe impl Send for Packet {}

impl std::fmt::Debug for Packet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Packet")
            .field("stream_index", &self.stream_index())
            .field("pts", &self.pts())
            .field("dts", &self.dts())
            .field("duration", &self.duration())
            .field("pos", &self.pos())
            .field("is_key", &self.is_key())
            .field("is_corrupt", &self.is_corrupt())
            .field("is_discard", &self.is_discard())
            .field("is_trusted", &self.is_trusted())
            .field("is_disposable", &self.is_disposable())
            .finish()
    }
}

impl Clone for Packet {
    fn clone(&self) -> Self {
        // Safety: `av_packet_clone` is safe to call.
        let clone = unsafe { av_packet_clone(self.0.as_ptr()) };

        // Safety: The pointer is valid.
        unsafe { Self::wrap(clone).expect("failed to clone packet") }
    }
}

impl Packet {
    /// Creates a new `Packet`.
    pub fn new() -> Result<Self, FfmpegError> {
        // Safety: `av_packet_alloc` is safe to call.
        let packet = unsafe { av_packet_alloc() };

        // Safety: The pointer is valid.
        unsafe { Self::wrap(packet) }.ok_or(FfmpegError::Alloc)
    }

    /// Wraps a pointer to a packet.
    ///
    /// # Safety
    /// `ptr` must be a valid pointer to a packet.
    unsafe fn wrap(ptr: *mut AVPacket) -> Option<Self> {
        SmartPtr::wrap_non_null(ptr, |ptr| av_packet_free(ptr)).map(Self)
    }

    /// Returns a pointer to the packet.
    pub const fn as_ptr(&self) -> *const AVPacket {
        self.0.as_ptr()
    }

    /// Returns a mutable pointer to the packet.
    pub const fn as_mut_ptr(&mut self) -> *mut AVPacket {
        self.0.as_mut_ptr()
    }

    /// Returns the stream index of the packet.
    pub const fn stream_index(&self) -> i32 {
        self.0.as_deref_except().stream_index
    }

    /// Sets the stream index of the packet.
    pub const fn set_stream_index(&mut self, stream_index: i32) {
        self.0.as_deref_mut_except().stream_index = stream_index as _;
    }

    /// Returns the presentation timestamp of the packet.
    pub const fn pts(&self) -> Option<i64> {
        check_i64(self.0.as_deref_except().pts)
    }

    /// Sets the presentation timestamp of the packet.
    pub const fn set_pts(&mut self, pts: Option<i64>) {
        self.0.as_deref_mut_except().pts = or_nopts(pts);
    }

    /// Returns the decoding timestamp of the packet.
    pub const fn dts(&self) -> Option<i64> {
        check_i64(self.0.as_deref_except().dts)
    }

    /// Sets the decoding timestamp of the packet.
    pub const fn set_dts(&mut self, dts: Option<i64>) {
        self.0.as_deref_mut_except().dts = or_nopts(dts);
    }

    /// Returns the duration of the packet.
    pub const fn duration(&self) -> Option<i64> {
        check_i64(self.0.as_deref_except().duration)
    }

    /// Sets the duration of the packet.
    pub const fn set_duration(&mut self, duration: Option<i64>) {
        self.0.as_deref_mut_except().duration = or_nopts(duration);
    }

    /// Converts the timebase of the packet.
    pub fn convert_timebase(&mut self, from: AVRational, to: AVRational) {
        // Safety: av_rescale_q_rnd is safe to call
        self.set_pts(self.pts().map(|pts| {
            // Safety: av_rescale_q_rnd is safe to call
            unsafe { av_rescale_q_rnd(pts, from, to, AVRounding::AV_ROUND_NEAR_INF) }
        }));

        // Safety: av_rescale_q_rnd is safe to call
        self.set_dts(self.dts().map(|dts| {
            // Safety: av_rescale_q_rnd is safe to call
            unsafe { av_rescale_q_rnd(dts, from, to, AVRounding::AV_ROUND_NEAR_INF) }
        }));

        // Safety: av_rescale_q is safe to call
        self.set_duration(self.duration().map(|duration| unsafe { av_rescale_q(duration, from, to) }));
    }

    /// Returns the position of the packet.
    pub const fn pos(&self) -> Option<i64> {
        check_i64(self.0.as_deref_except().pos)
    }

    /// Sets the position of the packet.
    pub const fn set_pos(&mut self, pos: Option<i64>) {
        self.0.as_deref_mut_except().pos = or_nopts(pos);
    }

    /// Returns the data of the packet.
    pub const fn data(&self) -> &[u8] {
        if self.0.as_deref_except().size <= 0 {
            return &[];
        }

        // Safety: `self.0` is a valid pointer.
        unsafe { std::slice::from_raw_parts(self.0.as_deref_except().data, self.0.as_deref_except().size as usize) }
    }

    /// Returns whether the packet is a key frame.
    pub const fn is_key(&self) -> bool {
        self.0.as_deref_except().flags & AV_PKT_FLAG_KEY != 0
    }

    /// Returns whether the packet is corrupt.
    pub const fn is_corrupt(&self) -> bool {
        self.0.as_deref_except().flags & AV_PKT_FLAG_CORRUPT != 0
    }

    /// Returns whether the packet should be discarded.
    pub const fn is_discard(&self) -> bool {
        self.0.as_deref_except().flags & AV_PKT_FLAG_DISCARD != 0
    }

    /// Returns whether the packet is trusted.
    pub const fn is_trusted(&self) -> bool {
        self.0.as_deref_except().flags & AV_PKT_FLAG_TRUSTED != 0
    }

    /// Returns whether the packet is disposable.
    pub const fn is_disposable(&self) -> bool {
        self.0.as_deref_except().flags & AV_PKT_FLAG_DISPOSABLE != 0
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use ffmpeg_sys_next::AVRational;
    use insta::assert_debug_snapshot;

    use crate::packet::Packet;

    #[test]
    fn test_packet_clone_snapshot() {
        let mut original_packet = Packet::new().expect("Failed to create original Packet");
        original_packet.set_stream_index(1);
        original_packet.set_pts(Some(12345));
        original_packet.set_dts(Some(54321));
        original_packet.set_duration(Some(1000));
        original_packet.set_pos(Some(2000));

        let cloned_packet = original_packet.clone();

        assert_debug_snapshot!(cloned_packet, @r"
        Packet {
            stream_index: 1,
            pts: Some(
                12345,
            ),
            dts: Some(
                54321,
            ),
            duration: Some(
                1000,
            ),
            pos: Some(
                2000,
            ),
            is_key: false,
            is_corrupt: false,
            is_discard: false,
            is_trusted: false,
            is_disposable: false,
        }
        ");

        // ensure cloned packet is independent
        original_packet.set_pts(Some(99999));
        assert_ne!(
            cloned_packet.pts(),
            original_packet.pts(),
            "Expected cloned packet PTS to remain unchanged after modifying the original"
        );

        assert_debug_snapshot!(original_packet, @r"
        Packet {
            stream_index: 1,
            pts: Some(
                99999,
            ),
            dts: Some(
                54321,
            ),
            duration: Some(
                1000,
            ),
            pos: Some(
                2000,
            ),
            is_key: false,
            is_corrupt: false,
            is_discard: false,
            is_trusted: false,
            is_disposable: false,
        }
        ");
    }

    #[test]
    fn test_packet_as_ptr() {
        let packet = Packet::new().expect("Failed to create Packet");
        let raw_ptr = packet.as_ptr();

        assert!(!raw_ptr.is_null(), "Expected a non-null pointer from Packet::as_ptr");
        // Safety: `raw_ptr` is a valid pointer.
        unsafe {
            assert_eq!(
                (*raw_ptr).stream_index,
                0,
                "Expected the default stream_index to be 0 for a new Packet"
            );
        }
    }

    #[test]
    fn test_packet_rescale_timebase() {
        let mut packet = Packet::new().expect("Failed to create Packet");
        packet.set_pts(Some(1000));
        packet.set_dts(Some(900));
        packet.set_duration(Some(100));
        let from_time_base = AVRational { num: 1, den: 1000 };
        let to_time_base = AVRational { num: 1, den: 48000 };

        packet.convert_timebase(from_time_base, to_time_base);
        assert_debug_snapshot!(packet, @r"
        Packet {
            stream_index: 0,
            pts: Some(
                48000,
            ),
            dts: Some(
                43200,
            ),
            duration: Some(
                4800,
            ),
            pos: Some(
                -1,
            ),
            is_key: false,
            is_corrupt: false,
            is_discard: false,
            is_trusted: false,
            is_disposable: false,
        }
        ");
    }

    #[test]
    fn test_packet_data_empty() {
        let mut packet = Packet::new().expect("Failed to create Packet");
        // Safety: `packet.as_mut_ptr()` is a valid pointer.
        unsafe {
            let av_packet = packet.as_mut_ptr().as_mut().unwrap();
            av_packet.size = 0;
        }

        let data = packet.data();

        assert!(
            data.is_empty(),
            "Expected the data slice to be empty when packet size is zero"
        );
    }
}
