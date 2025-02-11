use bytes::Bytes;
use num_derive::FromPrimitive;
use scuffle_amf0::Amf0Value;

#[derive(Debug)]
pub enum RtmpMessageData<'a> {
    Amf0Command {
        command_name: Amf0Value<'a>,
        transaction_id: Amf0Value<'a>,
        command_object: Amf0Value<'a>,
        others: Vec<Amf0Value<'a>>,
    },
    Amf0Data {
        data: Bytes,
    },
    SetChunkSize {
        chunk_size: u32,
    },
    AudioData {
        data: Bytes,
    },
    VideoData {
        data: Bytes,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, FromPrimitive)]
#[repr(u8)]
pub enum MessageTypeID {
    SetChunkSize = 1,
    Abort = 2,
    Acknowledgement = 3,
    UserControlEvent = 4,
    WindowAcknowledgementSize = 5,
    SetPeerBandwidth = 6,
    Audio = 8,
    Video = 9,
    DataAMF3 = 15,
    SharedObjAMF3 = 16,
    CommandAMF3 = 17,
    DataAMF0 = 18,
    SharedObjAMF0 = 19,
    CommandAMF0 = 20,
    Aggregate = 22,
}
