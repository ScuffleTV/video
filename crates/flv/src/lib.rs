//! A pure Rust implementation of the FLV format, allowing for demuxing of FLV
//! files or streams.
//!
//! This does not support all FLV features (mainly those from FLV 10.1), however
//! it does support some newer features, from the enhanced FLV specification.
//!
//! ## Specifications
//!
//! - <https://github.com/veovera/enhanced-rtmp/blob/main/docs/legacy/video-file-format-v10-0-spec.pdf>
//! - <https://github.com/veovera/enhanced-rtmp/blob/main/docs/legacy/video-file-format-v10-1-spec.pdf>
//! - <https://github.com/veovera/enhanced-rtmp/blob/main/docs/enhanced/enhanced-rtmp-v1.pdf>
//! - <https://github.com/veovera/enhanced-rtmp/blob/main/docs/enhanced/enhanced-rtmp-v2.pdf>
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or
//! [Apache-2.0](./LICENSE.Apache-2.0) license. You can choose between one of
//! them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

pub mod aac;
pub mod audio;
pub mod av1;
pub mod avc;
pub mod file;
pub mod header;
pub mod hevc;
pub mod script;
pub mod tag;
pub mod video;

pub use crate::file::FlvFile;
pub use crate::header::FlvHeader;
pub use crate::tag::{FlvTag, FlvTagData, FlvTagType};

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::collections::HashMap;
    use std::io;
    use std::path::PathBuf;

    use bytes::Bytes;
    use scuffle_aac::{AudioObjectType, PartialAudioSpecificConfig};
    use scuffle_amf0::Amf0Value;
    use scuffle_av1::seq::SequenceHeaderObu;
    use scuffle_av1::ObuHeader;
    use scuffle_h264::{Sps, SpsExtended};

    use crate::aac::AacPacket;
    use crate::audio::{AudioData, AudioDataBody, SoundRate, SoundSize, SoundType};
    use crate::av1::Av1Packet;
    use crate::avc::AvcPacket;
    use crate::file::FlvFile;
    use crate::hevc::HevcPacket;
    use crate::script::ScriptData;
    use crate::tag::FlvTagData;
    use crate::video::{EnhancedPacket, FrameType, VideoFourCC, VideoTagBody, VideoTagHeader};

    #[test]
    fn test_demux_flv_avc_aac() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

        let data = Bytes::from(std::fs::read(dir.join("avc_aac.flv")).expect("failed to read file"));
        let mut reader = io::Cursor::new(data);

        let flv = FlvFile::demux(&mut reader).expect("failed to demux flv");

        assert_eq!(flv.header.version, 1);
        assert!(flv.header.has_audio);
        assert!(flv.header.has_video);
        assert_eq!(flv.header.extra.len(), 0);

        let mut tags = flv.tags.into_iter();

        // Metadata tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            // This is a metadata tag
            let script_data = match tag.data {
                FlvTagData::ScriptData(ScriptData { name, data }) => {
                    assert_eq!(name, "onMetaData");
                    data
                }
                _ => panic!("expected script data"),
            };

            // Script data should be an AMF0 object
            let object = match &script_data[0] {
                Amf0Value::Object(object) => object,
                _ => panic!("expected object"),
            };

            let object = object.iter().map(|(k, v)| (k.as_ref(), v)).collect::<HashMap<_, _>>();

            // Should have a audio sample size property
            let audio_sample_size = match object.get("audiosamplesize") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio sample size"),
            };

            assert_eq!(audio_sample_size, &16.0);

            // Should have a audio sample rate property
            let audio_sample_rate = match object.get("audiosamplerate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio sample rate"),
            };

            assert_eq!(audio_sample_rate, &48000.0);

            // Should have a stereo property
            let stereo = match object.get("stereo") {
                Some(Amf0Value::Boolean(boolean)) => boolean,
                _ => panic!("expected stereo"),
            };

            assert_eq!(stereo, &true);

            // Should have an audio codec id property
            let audio_codec_id = match object.get("audiocodecid") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio codec id"),
            };

            assert_eq!(audio_codec_id, &10.0); // AAC

            // Should have a video codec id property
            let video_codec_id = match object.get("videocodecid") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected video codec id"),
            };

            assert_eq!(video_codec_id, &7.0); // AVC

            // Should have a duration property
            let duration = match object.get("duration") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected duration"),
            };

            assert_eq!(duration, &1.088); // 1.088 seconds

            // Should have a width property
            let width = match object.get("width") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected width"),
            };

            assert_eq!(width, &3840.0);

            // Should have a height property
            let height = match object.get("height") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected height"),
            };

            assert_eq!(height, &2160.0);

            // Should have a framerate property
            let framerate = match object.get("framerate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected framerate"),
            };

            assert_eq!(framerate, &60.0);

            // Should have a videodatarate property
            match object.get("videodatarate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected videodatarate"),
            };

            // Should have a audiodatarate property
            match object.get("audiodatarate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audiodatarate"),
            };

            // Should have a minor version property
            let minor_version = match object.get("minor_version") {
                Some(Amf0Value::String(number)) => number,
                _ => panic!("expected minor version"),
            };

            assert_eq!(minor_version, "512");

            // Should have a major brand property
            let major_brand = match object.get("major_brand") {
                Some(Amf0Value::String(string)) => string,
                _ => panic!("expected major brand"),
            };

            assert_eq!(major_brand, "iso5");

            // Should have a compatible_brands property
            let compatible_brands = match object.get("compatible_brands") {
                Some(Amf0Value::String(string)) => string,
                _ => panic!("expected compatible brands"),
            };

            assert_eq!(compatible_brands, "iso5iso6mp41");
        }

        // Video Sequence Header Tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            // This is a video tag
            let (frame_type, video_data) = match tag.data {
                FlvTagData::Video(VideoTagHeader { frame_type, body }) => (frame_type, body),
                _ => panic!("expected video data"),
            };

            assert_eq!(frame_type, FrameType::Keyframe);

            // Video data should be an AVC sequence header
            let avc_decoder_configuration_record = match video_data {
                VideoTagBody::Avc(AvcPacket::SequenceHeader(data)) => data,
                _ => panic!("expected avc sequence header"),
            };

            // The avc sequence header should be able to be decoded into an avc decoder
            // configuration record
            assert_eq!(avc_decoder_configuration_record.profile_indication, 100);
            assert_eq!(avc_decoder_configuration_record.profile_compatibility, 0);
            assert_eq!(avc_decoder_configuration_record.level_indication, 51); // 5.1
            assert_eq!(avc_decoder_configuration_record.length_size_minus_one, 3);
            assert_eq!(avc_decoder_configuration_record.sps.len(), 1);
            assert_eq!(avc_decoder_configuration_record.pps.len(), 1);
            assert_eq!(avc_decoder_configuration_record.extended_config, None);

            let sps = &avc_decoder_configuration_record.sps[0];
            // SPS should be able to be decoded into a sequence parameter set
            let sps = Sps::parse(sps.clone()).expect("expected sequence parameter set");

            assert_eq!(sps.profile_idc, 100);
            assert_eq!(sps.level_idc, 51);
            assert_eq!(sps.width, 3840);
            assert_eq!(sps.height, 2160);
            assert_eq!(sps.frame_rate, 60.0);

            assert_eq!(
                sps.ext,
                Some(SpsExtended {
                    chroma_format_idc: 1,
                    bit_depth_luma_minus8: 0,
                    bit_depth_chroma_minus8: 0,
                })
            )
        }

        // Audio Sequence Header Tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            let (data, sound_rate, sound_size, sound_type) = match tag.data {
                FlvTagData::Audio(AudioData {
                    sound_rate,
                    sound_size,
                    sound_type,
                    body,
                }) => (body, sound_rate, sound_size, sound_type),
                _ => panic!("expected audio data"),
            };

            assert_eq!(sound_rate, SoundRate::Hz44000);
            assert_eq!(sound_size, SoundSize::Bit16);
            assert_eq!(sound_type, SoundType::Stereo);

            // Audio data should be an AAC sequence header
            let data = match data {
                AudioDataBody::Aac(AacPacket::SequenceHeader(data)) => data,
                _ => panic!("expected aac sequence header"),
            };

            // The aac sequence header should be able to be decoded into an aac decoder
            // configuration record
            let aac_decoder_configuration_record =
                PartialAudioSpecificConfig::parse(&data).expect("expected aac decoder configuration record");

            assert_eq!(
                aac_decoder_configuration_record.audio_object_type,
                AudioObjectType::AacLowComplexity
            );
            assert_eq!(aac_decoder_configuration_record.sampling_frequency, 48000);
            assert_eq!(aac_decoder_configuration_record.channel_configuration, 2);
        }

        // Rest of the tags should be video / audio data
        let mut last_timestamp = 0;
        let mut read_seq_end = false;
        for tag in tags {
            assert!(tag.timestamp_ms >= last_timestamp);
            assert_eq!(tag.stream_id, 0);

            last_timestamp = tag.timestamp_ms;

            match tag.data {
                FlvTagData::Audio(AudioData {
                    body,
                    sound_rate,
                    sound_size,
                    sound_type,
                }) => {
                    assert_eq!(sound_rate, SoundRate::Hz44000);
                    assert_eq!(sound_size, SoundSize::Bit16);
                    assert_eq!(sound_type, SoundType::Stereo);
                    match body {
                        AudioDataBody::Aac(AacPacket::Raw(data)) => data,
                        _ => panic!("expected aac raw packet"),
                    };
                }
                FlvTagData::Video(VideoTagHeader { frame_type, body }) => {
                    match frame_type {
                        FrameType::Keyframe => (),
                        FrameType::Interframe => (),
                        _ => panic!("expected keyframe or interframe"),
                    }

                    match body {
                        VideoTagBody::Avc(AvcPacket::Nalu { .. }) => assert!(!read_seq_end),
                        VideoTagBody::Avc(AvcPacket::EndOfSequence) => {
                            assert!(!read_seq_end);
                            read_seq_end = true;
                        }
                        _ => panic!("expected avc nalu packet: {:?}", body),
                    };
                }
                _ => panic!("expected audio data"),
            };
        }

        assert!(read_seq_end);
    }

    #[test]
    fn test_demux_flv_av1_aac() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

        let data = Bytes::from(std::fs::read(dir.join("av1_aac.flv")).expect("failed to read file"));
        let mut reader = io::Cursor::new(data);

        let flv = FlvFile::demux(&mut reader).expect("failed to demux flv");

        assert_eq!(flv.header.version, 1);
        assert!(flv.header.has_audio);
        assert!(flv.header.has_video);
        assert_eq!(flv.header.extra.len(), 0);

        let mut tags = flv.tags.into_iter();

        // Metadata tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            // This is a metadata tag
            let script_data = match tag.data {
                FlvTagData::ScriptData(ScriptData { name, data }) => {
                    assert_eq!(name, "onMetaData");
                    data
                }
                _ => panic!("expected script data"),
            };

            // Script data should be an AMF0 object
            let object = match &script_data[0] {
                Amf0Value::Object(object) => object,
                _ => panic!("expected object"),
            };

            let object = object.iter().map(|(k, v)| (k.as_ref(), v)).collect::<HashMap<_, _>>();

            // Should have a audio sample size property
            let audio_sample_size = match object.get("audiosamplesize") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio sample size"),
            };

            assert_eq!(audio_sample_size, &16.0);

            // Should have a audio sample rate property
            let audio_sample_rate = match object.get("audiosamplerate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio sample rate"),
            };

            assert_eq!(audio_sample_rate, &48000.0);

            // Should have a stereo property
            let stereo = match object.get("stereo") {
                Some(Amf0Value::Boolean(boolean)) => boolean,
                _ => panic!("expected stereo"),
            };

            assert_eq!(stereo, &true);

            // Should have an audio codec id property
            let audio_codec_id = match object.get("audiocodecid") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio codec id"),
            };

            assert_eq!(audio_codec_id, &10.0); // AAC

            // Should have a video codec id property
            let video_codec_id = match object.get("videocodecid") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected video codec id"),
            };

            assert_eq!(video_codec_id, &7.0); // AVC

            // Should have a duration property
            let duration = match object.get("duration") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected duration"),
            };

            assert_eq!(duration, &0.0); // 0 seconds (this was a live stream)

            // Should have a width property
            let width = match object.get("width") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected width"),
            };

            assert_eq!(width, &2560.0);

            // Should have a height property
            let height = match object.get("height") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected height"),
            };

            assert_eq!(height, &1440.0);

            // Should have a framerate property
            let framerate = match object.get("framerate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected framerate"),
            };

            assert_eq!(framerate, &144.0);

            // Should have a videodatarate property
            match object.get("videodatarate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected videodatarate"),
            };

            // Should have a audiodatarate property
            match object.get("audiodatarate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audiodatarate"),
            };
        }

        // Audio Sequence Header Tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            let (body, sound_rate, sound_size, sound_type) = match tag.data {
                FlvTagData::Audio(AudioData {
                    body,
                    sound_rate,
                    sound_size,
                    sound_type,
                }) => (body, sound_rate, sound_size, sound_type),
                _ => panic!("expected audio data"),
            };

            assert_eq!(sound_rate, SoundRate::Hz44000);
            assert_eq!(sound_size, SoundSize::Bit16);
            assert_eq!(sound_type, SoundType::Stereo);

            // Audio data should be an AAC sequence header
            let data = match body {
                AudioDataBody::Aac(AacPacket::SequenceHeader(data)) => data,
                _ => panic!("expected aac sequence header"),
            };

            // The aac sequence header should be able to be decoded into an aac decoder
            // configuration record
            let aac_decoder_configuration_record =
                PartialAudioSpecificConfig::parse(&data).expect("expected aac decoder configuration record");

            assert_eq!(
                aac_decoder_configuration_record.audio_object_type,
                AudioObjectType::AacLowComplexity
            );
            assert_eq!(aac_decoder_configuration_record.sampling_frequency, 48000);
            assert_eq!(aac_decoder_configuration_record.channel_configuration, 2);
        }

        // Video Sequence Header Tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            // This is a video tag
            let (frame_type, video_data) = match tag.data {
                FlvTagData::Video(VideoTagHeader { frame_type, body }) => (frame_type, body),
                _ => panic!("expected video data"),
            };

            assert_eq!(frame_type, FrameType::Keyframe);

            // Video data should be an AVC sequence header
            let config = match video_data {
                VideoTagBody::Enhanced(EnhancedPacket::Av1(Av1Packet::SequenceStart(config))) => config,
                _ => panic!("expected av1 sequence header found {:?}", video_data),
            };

            assert_eq!(config.chroma_sample_position, 0);
            assert!(config.chroma_subsampling_x); // 5.1
            assert!(config.chroma_subsampling_y);
            assert!(!config.high_bitdepth);
            assert!(!config.twelve_bit);

            let mut reader = std::io::Cursor::new(config.config_obu);

            let header = ObuHeader::parse(&mut reader).expect("expected obu header");

            let seq_obu = SequenceHeaderObu::parse(header, &mut reader).expect("expected sequence obu");

            assert_eq!(seq_obu.max_frame_height, 1440);
            assert_eq!(seq_obu.max_frame_width, 2560);
        }

        // Rest of the tags should be video / audio data
        let mut last_timestamp = 0;
        let mut read_seq_end = false;
        for tag in tags {
            assert!(tag.timestamp_ms >= last_timestamp || tag.timestamp_ms == 0); // Timestamps should be monotonically increasing or 0
            assert_eq!(tag.stream_id, 0);

            if tag.timestamp_ms != 0 {
                last_timestamp = tag.timestamp_ms;
            }

            match tag.data {
                FlvTagData::Audio(AudioData {
                    body,
                    sound_rate,
                    sound_size,
                    sound_type,
                }) => {
                    assert_eq!(sound_rate, SoundRate::Hz44000);
                    assert_eq!(sound_size, SoundSize::Bit16);
                    assert_eq!(sound_type, SoundType::Stereo);
                    match body {
                        AudioDataBody::Aac(AacPacket::Raw(data)) => data,
                        _ => panic!("expected aac raw packet"),
                    };
                }
                FlvTagData::Video(VideoTagHeader { frame_type, body }) => {
                    match frame_type {
                        FrameType::Keyframe => (),
                        FrameType::Interframe => (),
                        _ => panic!("expected keyframe or interframe"),
                    }

                    match body {
                        VideoTagBody::Enhanced(EnhancedPacket::Av1(Av1Packet::Raw(_))) => {
                            assert!(!read_seq_end)
                        }
                        VideoTagBody::Enhanced(EnhancedPacket::SequenceEnd { video_codec }) => {
                            assert!(!read_seq_end);
                            assert_eq!(video_codec, VideoFourCC::Av1);
                            read_seq_end = true;
                        }
                        _ => panic!("expected av1 raw packet: {:?}", body),
                    };
                }
                _ => panic!("expected audio data"),
            };
        }

        assert!(read_seq_end);
    }

    #[test]
    fn test_demux_flv_hevc_aac() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

        let data = Bytes::from(std::fs::read(dir.join("hevc_aac.flv")).expect("failed to read file"));
        let mut reader = io::Cursor::new(data);

        let flv = FlvFile::demux(&mut reader).expect("failed to demux flv");

        assert_eq!(flv.header.version, 1);
        assert!(flv.header.has_audio);
        assert!(flv.header.has_video);
        assert_eq!(flv.header.extra.len(), 0);

        let mut tags = flv.tags.into_iter();

        // Metadata tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            // This is a metadata tag
            let script_data = match tag.data {
                FlvTagData::ScriptData(ScriptData { name, data }) => {
                    assert_eq!(name, "onMetaData");
                    data
                }
                _ => panic!("expected script data"),
            };

            // Script data should be an AMF0 object
            let object = match &script_data[0] {
                Amf0Value::Object(object) => object,
                _ => panic!("expected object"),
            };

            let object = object.iter().map(|(k, v)| (k.as_ref(), v)).collect::<HashMap<_, _>>();

            // Should have a audio sample size property
            let audio_sample_size = match object.get("audiosamplesize") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio sample size"),
            };

            assert_eq!(audio_sample_size, &16.0);

            // Should have a audio sample rate property
            let audio_sample_rate = match object.get("audiosamplerate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio sample rate"),
            };

            assert_eq!(audio_sample_rate, &48000.0);

            // Should have a stereo property
            let stereo = match object.get("stereo") {
                Some(Amf0Value::Boolean(boolean)) => boolean,
                _ => panic!("expected stereo"),
            };

            assert_eq!(stereo, &true);

            // Should have an audio codec id property
            let audio_codec_id = match object.get("audiocodecid") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audio codec id"),
            };

            assert_eq!(audio_codec_id, &10.0); // AAC

            // Should have a video codec id property
            let video_codec_id = match object.get("videocodecid") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected video codec id"),
            };

            assert_eq!(video_codec_id, &7.0); // AVC

            // Should have a duration property
            let duration = match object.get("duration") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected duration"),
            };

            assert_eq!(duration, &0.0); // 0 seconds (this was a live stream)

            // Should have a width property
            let width = match object.get("width") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected width"),
            };

            assert_eq!(width, &2560.0);

            // Should have a height property
            let height = match object.get("height") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected height"),
            };

            assert_eq!(height, &1440.0);

            // Should have a framerate property
            let framerate = match object.get("framerate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected framerate"),
            };

            assert_eq!(framerate, &144.0);

            // Should have a videodatarate property
            match object.get("videodatarate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected videodatarate"),
            };

            // Should have a audiodatarate property
            match object.get("audiodatarate") {
                Some(Amf0Value::Number(number)) => number,
                _ => panic!("expected audiodatarate"),
            };
        }

        // Audio Sequence Header Tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            let (body, sound_rate, sound_size, sound_type) = match tag.data {
                FlvTagData::Audio(AudioData {
                    body,
                    sound_rate,
                    sound_size,
                    sound_type,
                }) => (body, sound_rate, sound_size, sound_type),
                _ => panic!("expected audio data"),
            };

            assert_eq!(sound_rate, SoundRate::Hz44000);
            assert_eq!(sound_size, SoundSize::Bit16);
            assert_eq!(sound_type, SoundType::Stereo);

            // Audio data should be an AAC sequence header
            let data = match body {
                AudioDataBody::Aac(AacPacket::SequenceHeader(data)) => data,
                _ => panic!("expected aac sequence header"),
            };

            // The aac sequence header should be able to be decoded into an aac decoder
            // configuration record
            let aac_decoder_configuration_record =
                PartialAudioSpecificConfig::parse(&data).expect("expected aac decoder configuration record");

            assert_eq!(
                aac_decoder_configuration_record.audio_object_type,
                AudioObjectType::AacLowComplexity
            );
            assert_eq!(aac_decoder_configuration_record.sampling_frequency, 48000);
            assert_eq!(aac_decoder_configuration_record.channel_configuration, 2);
        }

        // Video Sequence Header Tag
        {
            let tag = tags.next().expect("expected tag");
            assert_eq!(tag.timestamp_ms, 0);
            assert_eq!(tag.stream_id, 0);

            // This is a video tag
            let (frame_type, video_data) = match tag.data {
                FlvTagData::Video(VideoTagHeader { frame_type, body }) => (frame_type, body),
                _ => panic!("expected video data"),
            };

            assert_eq!(frame_type, FrameType::Keyframe);

            // Video data should be an AVC sequence header
            let config = match video_data {
                VideoTagBody::Enhanced(EnhancedPacket::Hevc(HevcPacket::SequenceStart(config))) => config,
                _ => panic!("expected hevc sequence header found {:?}", video_data),
            };

            assert_eq!(config.configuration_version, 1);
            assert_eq!(config.avg_frame_rate, 0);
            assert_eq!(config.constant_frame_rate, 0);
            assert_eq!(config.num_temporal_layers, 1);

            // We should be able to find a SPS NAL unit in the sequence header
            let Some(sps) = config
                .arrays
                .iter()
                .find(|a| a.nal_unit_type == scuffle_h265::NaluType::Sps)
                .and_then(|v| v.nalus.first())
            else {
                panic!("expected sps");
            };

            // We should be able to find a PPS NAL unit in the sequence header
            let Some(_) = config
                .arrays
                .iter()
                .find(|a| a.nal_unit_type == scuffle_h265::NaluType::Pps)
                .and_then(|v| v.nalus.first())
            else {
                panic!("expected pps");
            };

            // We should be able to decode the SPS NAL unit
            let sps = scuffle_h265::Sps::parse(sps.clone()).expect("expected sps");

            assert_eq!(sps.frame_rate, 144.0);
            assert_eq!(sps.width, 2560);
            assert_eq!(sps.height, 1440);
            assert_eq!(
                sps.color_config,
                Some(scuffle_h265::ColorConfig {
                    full_range: false,
                    color_primaries: 1,
                    transfer_characteristics: 1,
                    matrix_coefficients: 1,
                })
            )
        }

        // Rest of the tags should be video / audio data
        let mut last_timestamp = 0;
        let mut read_seq_end = false;
        for tag in tags {
            assert!(tag.timestamp_ms >= last_timestamp || tag.timestamp_ms == 0); // Timestamps should be monotonically increasing or 0
            assert_eq!(tag.stream_id, 0);

            if tag.timestamp_ms != 0 {
                last_timestamp = tag.timestamp_ms;
            }

            match tag.data {
                FlvTagData::Audio(AudioData {
                    body,
                    sound_rate,
                    sound_size,
                    sound_type,
                }) => {
                    assert_eq!(sound_rate, SoundRate::Hz44000);
                    assert_eq!(sound_size, SoundSize::Bit16);
                    assert_eq!(sound_type, SoundType::Stereo);
                    match body {
                        AudioDataBody::Aac(AacPacket::Raw(data)) => data,
                        _ => panic!("expected aac raw packet"),
                    };
                }
                FlvTagData::Video(VideoTagHeader { frame_type, body }) => {
                    match frame_type {
                        FrameType::Keyframe => (),
                        FrameType::Interframe => (),
                        _ => panic!("expected keyframe or interframe"),
                    }

                    match body {
                        VideoTagBody::Enhanced(EnhancedPacket::Hevc(HevcPacket::Nalu { .. })) => assert!(!read_seq_end),
                        VideoTagBody::Enhanced(EnhancedPacket::SequenceEnd { video_codec }) => {
                            assert!(!read_seq_end);
                            assert_eq!(video_codec, VideoFourCC::Hevc);
                            read_seq_end = true;
                        }
                        _ => panic!("expected hevc nalu packet: {:?}", body),
                    };
                }
                _ => panic!("expected audio data"),
            };
        }

        assert!(read_seq_end);
    }
}
