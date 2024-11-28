#![allow(clippy::single_match)]

use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::io;

use amf0::Amf0Value;
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes};
use bytesio::bytes_writer::BytesWriter;
use flv::{
	AacPacket, Av1Packet, AvcPacket, EnhancedPacket, FlvTag, FlvTagAudioData, FlvTagData, FlvTagVideoData, FrameType,
	HevcPacket, SoundType,
};
use mp4::codec::{AudioCodec, VideoCodec};
use mp4::types::ftyp::{FourCC, Ftyp};
use mp4::types::hdlr::{HandlerType, Hdlr};
use mp4::types::mdat::Mdat;
use mp4::types::mdhd::Mdhd;
use mp4::types::mdia::Mdia;
use mp4::types::mfhd::Mfhd;
use mp4::types::minf::Minf;
use mp4::types::moof::Moof;
use mp4::types::moov::Moov;
use mp4::types::mvex::Mvex;
use mp4::types::mvhd::Mvhd;
use mp4::types::smhd::Smhd;
use mp4::types::stbl::Stbl;
use mp4::types::stco::Stco;
use mp4::types::stsc::Stsc;
use mp4::types::stsd::Stsd;
use mp4::types::stsz::Stsz;
use mp4::types::stts::Stts;
use mp4::types::tfdt::Tfdt;
use mp4::types::tfhd::Tfhd;
use mp4::types::tkhd::Tkhd;
use mp4::types::traf::Traf;
use mp4::types::trak::Trak;
use mp4::types::trex::Trex;
use mp4::types::trun::Trun;
use mp4::types::vmhd::Vmhd;
use mp4::BoxType;

mod codecs;
mod define;
mod errors;

pub use define::*;
pub use errors::TransmuxError;

#[derive(Debug, Clone)]
pub struct Transmuxer {
	// These durations are measured in timescales
	/// sample_freq * 1000
	audio_duration: u64,
	/// fps * 1000
	video_duration: u64,
	sequence_number: u32,
	last_video_timestamp: u32,
	settings: Option<(VideoSettings, AudioSettings)>,
	tags: VecDeque<FlvTag>,
}

impl Default for Transmuxer {
	fn default() -> Self {
		Self::new()
	}
}

impl Transmuxer {
	pub fn new() -> Self {
		Self {
			sequence_number: 1,
			tags: VecDeque::new(),
			audio_duration: 0,
			video_duration: 0,
			last_video_timestamp: 0,
			settings: None,
		}
	}

	/// Feed raw FLV data to the transmuxer.
	pub fn demux(&mut self, data: Bytes) -> Result<(), TransmuxError> {
		let mut cursor = io::Cursor::new(data);
		while cursor.has_remaining() {
			cursor.read_u32::<BigEndian>()?; // previous tag size
			if !cursor.has_remaining() {
				break;
			}

			let tag = flv::FlvTag::demux(&mut cursor)?;
			self.tags.push_back(tag);
		}

		Ok(())
	}

	/// Feed a single FLV tag to the transmuxer.
	pub fn add_tag(&mut self, tag: FlvTag) {
		self.tags.push_back(tag);
	}

	/// Get the next transmuxed packet. This will return `None` if there is not
	/// enough data to create a packet.
	pub fn mux(&mut self) -> Result<Option<TransmuxResult>, TransmuxError> {
		let mut writer = BytesWriter::default();

		let Some((video_settings, _)) = &self.settings else {
			let Some((video_settings, audio_settings)) = self.init_sequence(&mut writer)? else {
				if self.tags.len() > 30 {
					// We are clearly not getting any sequence headers, so we should just give up
					return Err(TransmuxError::NoSequenceHeaders);
				}

				// We don't have enough tags to create an init segment yet
				return Ok(None);
			};

			self.settings = Some((video_settings.clone(), audio_settings.clone()));

			let data = writer.dispose();

			return Ok(Some(TransmuxResult::InitSegment {
				data,
				audio_settings,
				video_settings,
			}));
		};

		loop {
			let Some(tag) = self.tags.pop_front() else {
				return Ok(None);
			};

			let mdat_data;
			let total_duration;
			let trun_sample;
			let mut is_audio = false;
			let mut is_keyframe = false;

			let duration =
				if self.last_video_timestamp == 0 || tag.timestamp == 0 || tag.timestamp < self.last_video_timestamp {
					1000 // the first frame is always 1000 ticks where the timescale
					// is 1000 * fps.
				} else {
					// Since the delta is in milliseconds (ie 1/1000 of a second)
					// Rounding errors happen. Our presision is only 1/1000 of a second.
					// So if we have a 30fps video the delta should be 33.33ms (1000/30)
					// But we can only represent this as 33ms or 34ms. So we will get rounding
					// errors. To fix this we just check if the delta is 1 more or 1 less than the
					// expected delta. And if it is we just use the expected delta.
					// The reason we use a timescale which is 1000 * fps is because then we can
					// always represent the delta as an integer. If we use a timescale of 1000, we
					// would run into the same rounding errors.
					let delta = tag.timestamp as f64 - self.last_video_timestamp as f64;
					let expected_delta = 1000.0 / video_settings.framerate;
					if (delta - expected_delta).abs() <= 1.0 {
						1000
					} else {
						(delta * video_settings.framerate) as u32
					}
				};

			match tag.data {
				FlvTagData::Audio {
					data: FlvTagAudioData::Aac(AacPacket::Raw(data)),
					..
				} => {
					let (sample, duration) = codecs::aac::trun_sample(&data)?;

					trun_sample = sample;
					mdat_data = data;
					total_duration = duration;
					is_audio = true;
				}
				FlvTagData::Video {
					frame_type,
					data: FlvTagVideoData::Avc(AvcPacket::Nalu { composition_time, data }),
				} => {
					let composition_time = ((composition_time as f64 * video_settings.framerate) / 1000.0).floor() * 1000.0;

					let sample = codecs::avc::trun_sample(frame_type, composition_time as u32, duration, &data)?;

					trun_sample = sample;
					total_duration = duration;
					mdat_data = data;

					is_keyframe = frame_type == FrameType::Keyframe;
				}
				FlvTagData::Video {
					frame_type,
					data: FlvTagVideoData::Enhanced(EnhancedPacket::Av1(Av1Packet::Raw(data))),
				} => {
					let sample = codecs::av1::trun_sample(frame_type, duration, &data)?;

					trun_sample = sample;
					total_duration = duration;
					mdat_data = data;

					is_keyframe = frame_type == FrameType::Keyframe;
				}
				FlvTagData::Video {
					frame_type,
					data: FlvTagVideoData::Enhanced(EnhancedPacket::Hevc(HevcPacket::Nalu { composition_time, data })),
				} => {
					let composition_time =
						((composition_time.unwrap_or_default() as f64 * video_settings.framerate) / 1000.0).floor() * 1000.0;

					let sample = codecs::hevc::trun_sample(frame_type, composition_time as i32, duration, &data)?;

					trun_sample = sample;
					total_duration = duration;
					mdat_data = data;

					is_keyframe = frame_type == FrameType::Keyframe;
				}
				_ => {
					// We don't support anything else
					continue;
				}
			}

			let trafs = {
				let (main_duration, main_id) = if is_audio {
					(self.audio_duration, 2)
				} else {
					(self.video_duration, 1)
				};

				let mut traf = Traf::new(
					Tfhd::new(main_id, None, None, None, None, None),
					Some(Trun::new(vec![trun_sample], None)),
					Some(Tfdt::new(main_duration)),
				);
				traf.optimize();

				vec![traf]
			};

			let mut moof = Moof::new(Mfhd::new(self.sequence_number), trafs);

			// We need to get the moof size so that we can set the data offsets.
			let moof_size = moof.size();

			// We just created the moof, and therefore we know that the first traf is the
			// video traf and the second traf is the audio traf. So we can just unwrap them
			// and set the data offsets.
			let traf = moof.traf.get_mut(0).expect("we just created the moof with a traf");

			// Again we know that these exist because we just created it.
			let trun = traf.trun.as_mut().expect("we just created the video traf with a trun");

			// We now define the offsets.
			// So the video offset will be the size of the moof + 8 bytes for the mdat
			// header.
			trun.data_offset = Some(moof_size as i32 + 8);

			// We then write the moof to the writer.
			moof.mux(&mut writer)?;

			// We create an mdat box and write it to the writer.
			Mdat::new(vec![mdat_data]).mux(&mut writer)?;

			// Increase our sequence number and duration.
			self.sequence_number += 1;

			if is_audio {
				self.audio_duration += total_duration as u64;
				return Ok(Some(TransmuxResult::MediaSegment(MediaSegment {
					data: writer.dispose(),
					ty: MediaType::Audio,
					keyframe: false,
					timestamp: self.audio_duration - total_duration as u64,
				})));
			} else {
				self.video_duration += total_duration as u64;
				self.last_video_timestamp = tag.timestamp;
				return Ok(Some(TransmuxResult::MediaSegment(MediaSegment {
					data: writer.dispose(),
					ty: MediaType::Video,
					keyframe: is_keyframe,
					timestamp: self.video_duration - total_duration as u64,
				})));
			}
		}
	}

	/// Internal function to find the tags we need to create the init segment.
	fn find_tags(
		&self,
	) -> (
		Option<VideoSequenceHeader>,
		Option<AudioSequenceHeader>,
		Option<HashMap<String, Amf0Value>>,
	) {
		let tags = self.tags.iter();
		let mut video_sequence_header = None;
		let mut audio_sequence_header = None;
		let mut scriptdata_tag = None;

		for tag in tags {
			if video_sequence_header.is_some() && audio_sequence_header.is_some() && scriptdata_tag.is_some() {
				break;
			}

			match &tag.data {
				FlvTagData::Video {
					frame_type: _,
					data: FlvTagVideoData::Avc(AvcPacket::SequenceHeader(data)),
				} => {
					video_sequence_header = Some(VideoSequenceHeader::Avc(data.clone()));
				}
				FlvTagData::Video {
					frame_type: _,
					data: FlvTagVideoData::Enhanced(EnhancedPacket::Av1(Av1Packet::SequenceStart(config))),
				} => {
					video_sequence_header = Some(VideoSequenceHeader::Av1(config.clone()));
				}
				FlvTagData::Video {
					frame_type: _,
					data: FlvTagVideoData::Enhanced(EnhancedPacket::Hevc(HevcPacket::SequenceStart(config))),
				} => {
					video_sequence_header = Some(VideoSequenceHeader::Hevc(config.clone()));
				}
				FlvTagData::Audio {
					sound_size,
					sound_type,
					sound_rate: _,
					data: FlvTagAudioData::Aac(AacPacket::SequenceHeader(data)),
				} => {
					audio_sequence_header = Some(AudioSequenceHeader {
						data: AudioSequenceHeaderData::Aac(data.clone()),
						sound_size: *sound_size,
						sound_type: *sound_type,
					});
				}
				FlvTagData::ScriptData { data, name } => {
					if name == "@setDataFrame" || name == "onMetaData" {
						let meta_object = data.iter().find(|v| matches!(v, Amf0Value::Object(_)));

						if let Some(Amf0Value::Object(meta_object)) = meta_object {
							scriptdata_tag = Some(meta_object.clone());
						}
					}
				}
				_ => {}
			}
		}

		(video_sequence_header, audio_sequence_header, scriptdata_tag)
	}

	/// Create the init segment.
	fn init_sequence(&mut self, writer: &mut BytesWriter) -> Result<Option<(VideoSettings, AudioSettings)>, TransmuxError> {
		// We need to find the tag that is the video sequence header
		// and the audio sequence header
		let (video_sequence_header, audio_sequence_header, scriptdata_tag) = self.find_tags();

		let Some(video_sequence_header) = video_sequence_header else {
			return Ok(None);
		};
		let Some(audio_sequence_header) = audio_sequence_header else {
			return Ok(None);
		};

		let video_codec;
		let audio_codec;
		let video_width;
		let video_height;
		let audio_channels;
		let audio_sample_rate;
		let mut video_fps = 0.0;

		let mut estimated_video_bitrate = 0;
		let mut estimated_audio_bitrate = 0;

		if let Some(scriptdata_tag) = scriptdata_tag {
			video_fps = scriptdata_tag
				.get("framerate")
				.and_then(|v| match v {
					Amf0Value::Number(v) => Some(*v),
					_ => None,
				})
				.unwrap_or(0.0);

			estimated_video_bitrate = scriptdata_tag
				.get("videodatarate")
				.and_then(|v| match v {
					Amf0Value::Number(v) => Some((*v * 1024.0) as u32),
					_ => None,
				})
				.unwrap_or(0);

			estimated_audio_bitrate = scriptdata_tag
				.get("audiodatarate")
				.and_then(|v| match v {
					Amf0Value::Number(v) => Some((*v * 1024.0) as u32),
					_ => None,
				})
				.unwrap_or(0);
		}

		let mut compatiable_brands = vec![FourCC::Iso5, FourCC::Iso6];

		let video_stsd_entry = match video_sequence_header {
			VideoSequenceHeader::Avc(config) => {
				compatiable_brands.push(FourCC::Avc1);
				video_codec = VideoCodec::Avc {
					constraint_set: config.profile_compatibility,
					level: config.level_indication,
					profile: config.profile_indication,
				};

				let (entry, sps) = codecs::avc::stsd_entry(config)?;
				if sps.frame_rate != 0.0 {
					video_fps = sps.frame_rate;
				}

				video_width = sps.width as u32;
				video_height = sps.height as u32;

				entry
			}
			VideoSequenceHeader::Av1(config) => {
				compatiable_brands.push(FourCC::Av01);
				let (entry, seq_obu) = codecs::av1::stsd_entry(config)?;

				video_height = seq_obu.max_frame_height as u32;
				video_width = seq_obu.max_frame_width as u32;

				let op_point = &seq_obu.operating_points[0];

				video_codec = VideoCodec::Av1 {
					profile: seq_obu.seq_profile,
					level: op_point.seq_level_idx,
					tier: op_point.seq_tier,
					depth: seq_obu.color_config.bit_depth as u8,
					monochrome: seq_obu.color_config.mono_chrome,
					sub_sampling_x: seq_obu.color_config.subsampling_x,
					sub_sampling_y: seq_obu.color_config.subsampling_y,
					color_primaries: seq_obu.color_config.color_primaries,
					transfer_characteristics: seq_obu.color_config.transfer_characteristics,
					matrix_coefficients: seq_obu.color_config.matrix_coefficients,
					full_range_flag: seq_obu.color_config.full_color_range,
				};

				entry
			}
			VideoSequenceHeader::Hevc(config) => {
				compatiable_brands.push(FourCC::Hev1);
				video_codec = VideoCodec::Hevc {
					constraint_indicator: config.general_constraint_indicator_flags,
					level: config.general_level_idc,
					profile: config.general_profile_idc,
					profile_compatibility: config.general_profile_compatibility_flags,
					tier: config.general_tier_flag,
					general_profile_space: config.general_profile_space,
				};

				let (entry, sps) = codecs::hevc::stsd_entry(config)?;
				if sps.frame_rate != 0.0 {
					video_fps = sps.frame_rate;
				}

				video_width = sps.width as u32;
				video_height = sps.height as u32;

				entry
			}
		};

		let audio_stsd_entry = match audio_sequence_header.data {
			AudioSequenceHeaderData::Aac(data) => {
				compatiable_brands.push(FourCC::Mp41);
				let (entry, config) =
					codecs::aac::stsd_entry(audio_sequence_header.sound_size, audio_sequence_header.sound_type, data)?;

				audio_sample_rate = config.sampling_frequency;

				audio_codec = AudioCodec::Aac {
					object_type: config.audio_object_type,
				};
				audio_channels = match audio_sequence_header.sound_type {
					SoundType::Mono => 1,
					SoundType::Stereo => 2,
				};

				entry
			}
		};

		if video_fps == 0.0 {
			return Err(TransmuxError::InvalidVideoFrameRate);
		}

		if video_width == 0 || video_height == 0 {
			return Err(TransmuxError::InvalidVideoDimensions);
		}

		if audio_sample_rate == 0 {
			return Err(TransmuxError::InvalidAudioSampleRate);
		}

		// The reason we multiply the FPS by 1000 is to avoid rounding errors
		// Consider If we had a video with a framerate of 30fps. That would imply each
		// frame is 33.333333ms So we are limited to a u32 and therefore we could only
		// represent 33.333333ms as 33ms. So this value is 30 * 1000 = 30000 timescale
		// units per second, making each frame 1000 units long instead of 33ms long.
		let video_timescale = (1000.0 * video_fps) as u32;

		Ftyp::new(FourCC::Iso5, 512, compatiable_brands).mux(writer)?;
		Moov::new(
			Mvhd::new(0, 0, 1000, 0, 1),
			vec![
				Trak::new(
					Tkhd::new(0, 0, 1, 0, Some((video_width, video_height))),
					None,
					Mdia::new(
						Mdhd::new(0, 0, video_timescale, 0),
						Hdlr::new(HandlerType::Vide, "VideoHandler".to_string()),
						Minf::new(
							Stbl::new(
								Stsd::new(vec![video_stsd_entry]),
								Stts::new(vec![]),
								Stsc::new(vec![]),
								Stco::new(vec![]),
								Some(Stsz::new(0, vec![])),
							),
							Some(Vmhd::new()),
							None,
						),
					),
				),
				Trak::new(
					Tkhd::new(0, 0, 2, 0, None),
					None,
					Mdia::new(
						Mdhd::new(0, 0, audio_sample_rate, 0),
						Hdlr::new(HandlerType::Soun, "SoundHandler".to_string()),
						Minf::new(
							Stbl::new(
								Stsd::new(vec![audio_stsd_entry]),
								Stts::new(vec![]),
								Stsc::new(vec![]),
								Stco::new(vec![]),
								Some(Stsz::new(0, vec![])),
							),
							None,
							Some(Smhd::new()),
						),
					),
				),
			],
			Some(Mvex::new(vec![Trex::new(1), Trex::new(2)], None)),
		)
		.mux(writer)?;

		Ok(Some((
			VideoSettings {
				width: video_width,
				height: video_height,
				framerate: video_fps,
				codec: video_codec,
				bitrate: estimated_video_bitrate,
				timescale: video_timescale,
			},
			AudioSettings {
				codec: audio_codec,
				sample_rate: audio_sample_rate,
				channels: audio_channels,
				bitrate: estimated_audio_bitrate,
				timescale: audio_sample_rate,
			},
		)))
	}
}

#[cfg(test)]
mod tests;
