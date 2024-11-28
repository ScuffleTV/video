use std::io::{
	Write, {self},
};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use av1::AV1CodecConfigurationRecord;
use bytes::{Buf, Bytes};
use fixed::FixedI32;
use h264::AVCDecoderConfigurationRecord;
use h265::{HEVCDecoderConfigurationRecord, NaluArray, NaluType};

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::types::avc1::Avc1;
use crate::boxes::types::avcc::AvcC;
use crate::boxes::types::btrt::Btrt;
use crate::boxes::types::dinf::Dinf;
use crate::boxes::types::dref::Dref;
use crate::boxes::types::edts::Edts;
use crate::boxes::types::elst::{Elst, ElstEntry};
use crate::boxes::types::esds::descriptor::header::DescriptorHeader;
use crate::boxes::types::esds::descriptor::types::decoder_config::DecoderConfigDescriptor;
use crate::boxes::types::esds::descriptor::types::decoder_specific_info::DecoderSpecificInfoDescriptor;
use crate::boxes::types::esds::descriptor::types::es::EsDescriptor;
use crate::boxes::types::esds::descriptor::types::sl_config::SLConfigDescriptor;
use crate::boxes::types::esds::Esds;
use crate::boxes::types::ftyp::{FourCC, Ftyp};
use crate::boxes::types::hdlr::{HandlerType, Hdlr};
use crate::boxes::types::mdhd::Mdhd;
use crate::boxes::types::mfhd::Mfhd;
use crate::boxes::types::minf::Minf;
use crate::boxes::types::moof::Moof;
use crate::boxes::types::mp4a::Mp4a;
use crate::boxes::types::mvhd::Mvhd;
use crate::boxes::types::pasp::Pasp;
use crate::boxes::types::smhd::Smhd;
use crate::boxes::types::stbl::Stbl;
use crate::boxes::types::stco::Stco;
use crate::boxes::types::stsc::Stsc;
use crate::boxes::types::stsd::{AudioSampleEntry, SampleEntry, Stsd, VisualSampleEntry};
use crate::boxes::types::stsz::Stsz;
use crate::boxes::types::stts::Stts;
use crate::boxes::types::tfdt::Tfdt;
use crate::boxes::types::tfhd::Tfhd;
use crate::boxes::types::tkhd::Tkhd;
use crate::boxes::types::traf::Traf;
use crate::boxes::types::trun::{Trun, TrunSample};
use crate::boxes::types::url::Url;
use crate::boxes::types::vmhd::Vmhd;
use crate::boxes::DynBox;
use crate::types::av01::Av01;
use crate::types::av1c::Av1C;
use crate::types::colr::{ColorType, Colr};
use crate::types::hev1::Hev1;
use crate::types::hvcc::HvcC;
use crate::types::mvex::Mvex;
use crate::types::trex::Trex;

#[test]
fn test_demux_avc_aac() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

	let data = std::fs::read(dir.join("avc_aac_fragmented.mp4").to_str().unwrap()).unwrap();

	let mut boxes = Vec::new();
	let mut reader = io::Cursor::new(data.into());

	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		boxes.push(box_);
	}

	// The values we assert against are taken from `https://mlynoteka.mlyn.org/mp4parser/`
	// We assume this to be a correct implementation of the MP4 format.

	// ftyp
	{
		let ftyp = boxes[0].as_ftyp().expect("ftyp");
		assert_eq!(
			ftyp,
			&Ftyp {
				header: BoxHeader { box_type: *b"ftyp" },
				major_brand: FourCC::Iso5,
				minor_version: 512,
				compatible_brands: vec![FourCC::Iso5, FourCC::Iso6, FourCC::Mp41],
			}
		);
	}

	// moov
	{
		let moov = boxes[1].as_moov().expect("moov");
		assert_eq!(
			moov.mvhd,
			Mvhd {
				header: FullBoxHeader {
					header: BoxHeader { box_type: *b"mvhd" },
					version: 0,
					flags: 0,
				},
				creation_time: 0,
				modification_time: 0,
				timescale: 1000,
				duration: 0,
				rate: FixedI32::from_num(1),
				volume: 1.into(),
				matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
				next_track_id: 2,
				reserved: 0,
				pre_defined: [0; 6],
				reserved2: [0; 2],
			}
		);

		// video track
		{
			let video_trak = &moov.traks[0];
			assert_eq!(
				video_trak.tkhd,
				Tkhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"tkhd" },
						version: 0,
						flags: 3,
					},
					creation_time: 0,
					modification_time: 0,
					track_id: 1,
					reserved: 0,
					duration: 0,
					reserved2: [0; 2],
					layer: 0,
					alternate_group: 0,
					volume: 0.into(),
					reserved3: 0,
					matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
					width: FixedI32::from_num(3840),
					height: FixedI32::from_num(2160),
				}
			);

			assert_eq!(
				video_trak.edts,
				Some(Edts {
					header: BoxHeader { box_type: *b"edts" },
					elst: Some(Elst {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"elst" },
							version: 0,
							flags: 0,
						},
						entries: vec![
							ElstEntry {
								segment_duration: 33,
								media_time: -1,
								media_rate_integer: 1,
								media_rate_fraction: 0,
							},
							ElstEntry {
								segment_duration: 0,
								media_time: 2000,
								media_rate_integer: 1,
								media_rate_fraction: 0,
							},
						],
					}),
					unknown: vec![],
				})
			);

			assert_eq!(
				video_trak.mdia.mdhd,
				Mdhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mdhd" },
						version: 0,
						flags: 0,
					},
					creation_time: 0,
					modification_time: 0,
					timescale: 60000,
					duration: 0,
					language: 21956,
					pre_defined: 0,
				}
			);

			assert_eq!(
				video_trak.mdia.hdlr,
				Hdlr {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"hdlr" },
						version: 0,
						flags: 0,
					},
					handler_type: HandlerType::Vide,
					reserved: [0; 3],
					name: "GPAC ISO Video Handler".into(),
					pre_defined: 0,
				}
			);

			assert_eq!(
				video_trak.mdia.minf.vmhd,
				Some(Vmhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"vmhd" },
						version: 0,
						flags: 1,
					},
					graphics_mode: 0,
					opcolor: [0, 0, 0],
				})
			);

			assert_eq!(
				video_trak.mdia.minf.dinf,
				Dinf {
					header: BoxHeader { box_type: *b"dinf" },
					unknown: vec![],
					dref: Dref {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"dref" },
							version: 0,
							flags: 0,
						},
						entries: vec![DynBox::Url(Url {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"url " },
								version: 0,
								flags: 1,
							},
							location: None,
						})],
					},
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsd,
				Stsd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsd" },
						version: 0,
						flags: 0,
					},
					entries: vec![DynBox::Avc1(Avc1 {
						header: BoxHeader { box_type: *b"avc1" },
						visual_sample_entry: SampleEntry::<VisualSampleEntry> {
							reserved: [0; 6],
							data_reference_index: 1,
							extension: VisualSampleEntry {
								clap: None,
								colr: None,
								compressorname: [0; 32],
								depth: 24,
								frame_count: 1,
								width: 3840,
								height: 2160,
								horizresolution: 4718592,
								vertresolution: 4718592,
								pre_defined2: [0; 3],
								pre_defined: 0,
								pre_defined3: -1,
								reserved: 0,
								reserved2: 0,
								pasp: Some(Pasp {
									header: BoxHeader { box_type: *b"pasp" },
									h_spacing: 1,
									v_spacing: 1,
								}),
							}
						},
						avcc: AvcC {
							header: BoxHeader { box_type: *b"avcC" },
							avc_decoder_configuration_record: AVCDecoderConfigurationRecord {
								level_indication: 51,
								profile_indication: 100,
								configuration_version: 1,
								length_size_minus_one: 3,
								profile_compatibility: 0,
								extended_config: None,
								sps: vec![Bytes::from(vec![
									103, 100, 0, 51, 172, 202, 80, 15, 0, 16, 251, 1, 16, 0, 0, 3, 0, 16, 0, 0, 7, 136, 241,
									131, 25, 96
								])],
								pps: vec![Bytes::from(vec![104, 233, 59, 44, 139])],
							},
						},
						btrt: Some(Btrt {
							header: BoxHeader { box_type: *b"btrt" },
							avg_bitrate: 8002648,
							max_bitrate: 8002648,
							buffer_size_db: 0,
						}),
						unknown: vec![],
					})],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stts,
				Stts {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stts" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsc,
				Stsc {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsc" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsz,
				Some(Stsz {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsz" },
						version: 0,
						flags: 0,
					},
					sample_size: 0,
					samples: vec![],
				})
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stco,
				Stco {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stco" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(video_trak.mdia.minf.stbl.co64, None);

			assert_eq!(video_trak.mdia.minf.stbl.ctts, None);

			assert_eq!(video_trak.mdia.minf.stbl.padb, None);

			assert_eq!(video_trak.mdia.minf.stbl.sbgp, None);

			assert_eq!(video_trak.mdia.minf.stbl.sdtp, None);

			assert_eq!(video_trak.mdia.minf.stbl.stdp, None);

			assert_eq!(video_trak.mdia.minf.stbl.stsh, None);

			assert_eq!(video_trak.mdia.minf.stbl.stss, None);

			assert_eq!(video_trak.mdia.minf.stbl.stz2, None);

			assert_eq!(video_trak.mdia.minf.stbl.subs, None);
		}

		// audio track
		{
			let audio_trak = &moov.traks[1];
			assert_eq!(
				audio_trak.tkhd,
				Tkhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"tkhd" },
						version: 0,
						flags: 3,
					},
					creation_time: 0,
					modification_time: 0,
					track_id: 2,
					duration: 0,
					layer: 0,
					alternate_group: 1,
					volume: 1.into(),
					matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
					width: FixedI32::from_bits(0),
					height: FixedI32::from_bits(0),
					reserved: 0,
					reserved2: [0; 2],
					reserved3: 0,
				}
			);

			assert_eq!(
				audio_trak.edts,
				Some(Edts {
					header: BoxHeader { box_type: *b"edts" },
					elst: Some(Elst {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"elst" },
							version: 0,
							flags: 0,
						},
						entries: vec![ElstEntry {
							media_rate_fraction: 0,
							media_time: 1024,
							media_rate_integer: 1,
							segment_duration: 0,
						}],
					}),
					unknown: vec![],
				})
			);

			assert_eq!(
				audio_trak.mdia.mdhd,
				Mdhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mdhd" },
						version: 0,
						flags: 0,
					},
					creation_time: 0,
					modification_time: 0,
					timescale: 48000,
					duration: 0,
					language: 21956,
					pre_defined: 0,
				}
			);

			assert_eq!(
				audio_trak.mdia.hdlr,
				Hdlr {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"hdlr" },
						version: 0,
						flags: 0,
					},
					handler_type: (*b"soun").into(),
					name: "GPAC ISO Audio Handler".to_string(),
					pre_defined: 0,
					reserved: [0; 3],
				}
			);

			assert_eq!(
				audio_trak.mdia.minf,
				Minf {
					header: BoxHeader { box_type: *b"minf" },
					smhd: Some(Smhd {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"smhd" },
							version: 0,
							flags: 0,
						},
						balance: 0.into(),
						reserved: 0,
					}),
					dinf: Dinf {
						header: BoxHeader { box_type: *b"dinf" },
						dref: Dref {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"dref" },
								version: 0,
								flags: 0,
							},
							entries: vec![DynBox::Url(Url {
								header: FullBoxHeader {
									header: BoxHeader { box_type: *b"url " },
									version: 0,
									flags: 1,
								},
								location: None,
							})],
						},
						unknown: vec![],
					},
					stbl: Stbl {
						header: BoxHeader { box_type: *b"stbl" },
						stsd: Stsd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsd" },
								version: 0,
								flags: 0,
							},
							entries: vec![DynBox::Mp4a(Mp4a {
								header: BoxHeader { box_type: *b"mp4a" },
								audio_sample_entry: SampleEntry::<AudioSampleEntry> {
									data_reference_index: 1,
									reserved: [0; 6],
									extension: AudioSampleEntry {
										channel_count: 2,
										pre_defined: 0,
										reserved2: 0,
										sample_size: 16,
										reserved: [0; 2],
										sample_rate: 48000,
									},
								},
								esds: Esds {
									header: FullBoxHeader {
										header: BoxHeader { box_type: *b"esds" },
										version: 0,
										flags: 0,
									},
									es_descriptor: EsDescriptor {
										header: DescriptorHeader { tag: 3.into() },
										es_id: 2,
										depends_on_es_id: None,
										ocr_es_id: None,
										stream_priority: 0,
										url: None,
										decoder_config: Some(DecoderConfigDescriptor {
											header: DescriptorHeader { tag: 4.into() },
											avg_bitrate: 128000,
											buffer_size_db: 0,
											max_bitrate: 128000,
											reserved: 1,
											stream_type: 5,
											object_type_indication: 64,
											up_stream: false,
											decoder_specific_info: Some(DecoderSpecificInfoDescriptor {
												header: DescriptorHeader { tag: 5.into() },
												data: Bytes::from_static(b"\x11\x90V\xe5\0"),
											}),
											unknown: vec![],
										}),
										sl_config: Some(SLConfigDescriptor {
											header: DescriptorHeader { tag: 6.into() },
											predefined: 2,
											data: Bytes::new(),
										}),
										unknown: vec![],
									},
									unknown: vec![],
								},
								btrt: Some(Btrt {
									header: BoxHeader { box_type: *b"btrt" },
									buffer_size_db: 0,
									max_bitrate: 128000,
									avg_bitrate: 128000,
								}),
								unknown: vec![],
							})],
						},
						co64: None,
						ctts: None,
						padb: None,
						sbgp: None,
						sdtp: None,
						stdp: None,
						stco: Stco {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stco" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stsc: Stsc {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsc" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stsh: None,
						stss: None,
						stsz: Some(Stsz {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsz" },
								version: 0,
								flags: 0,
							},
							sample_size: 0,
							samples: vec![],
						}),
						stts: Stts {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stts" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stz2: None,
						subs: None,
						unknown: vec![],
					},
					hmhd: None,
					nmhd: None,
					vmhd: None,
					unknown: vec![],
				}
			);
		}

		// mvex
		assert_eq!(
			moov.mvex,
			Some(Mvex {
				header: BoxHeader { box_type: *b"mvex" },
				mehd: None,
				trex: vec![
					Trex {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"trex" },
							version: 0,
							flags: 0,
						},
						track_id: 1,
						default_sample_description_index: 1,
						default_sample_duration: 0,
						default_sample_size: 0,
						default_sample_flags: 0,
					},
					Trex {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"trex" },
							version: 0,
							flags: 0,
						},
						track_id: 2,
						default_sample_description_index: 1,
						default_sample_duration: 0,
						default_sample_size: 0,
						default_sample_flags: 0,
					},
				],
				unknown: vec![],
			})
		)
	}

	// moof
	{
		let moof = boxes[2].as_moof().expect("moof");
		assert_eq!(
			moof,
			&Moof {
				header: BoxHeader { box_type: *b"moof" },
				mfhd: Mfhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mfhd" },
						version: 0,
						flags: 0,
					},
					sequence_number: 1,
				},
				traf: vec![
					Traf {
						header: BoxHeader { box_type: *b"traf" },
						tfhd: Tfhd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfhd" },
								version: 0,
								flags: Tfhd::DEFAULT_BASE_IS_MOOF_FLAG
									| Tfhd::DEFAULT_SAMPLE_FLAGS_FLAG | Tfhd::DEFAULT_SAMPLE_SIZE_FLAG
									| Tfhd::DEFAULT_SAMPLE_DURATION_FLAG,
							},
							track_id: 1,
							base_data_offset: None,
							sample_description_index: None,
							default_sample_duration: Some(1000),
							default_sample_size: Some(2232),
							default_sample_flags: Some(0x1010000.into()),
						},
						tfdt: Some(Tfdt {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfdt" },
								version: 1,
								flags: 0,
							},
							base_media_decode_time: 0,
						}),
						trun: Some(Trun {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"trun" },
								version: 0,
								flags: Trun::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET
									| Trun::FLAG_FIRST_SAMPLE_FLAGS | Trun::FLAG_DATA_OFFSET
									| Trun::FLAG_SAMPLE_SIZE,
							},
							data_offset: Some(356),
							first_sample_flags: Some(33554432.into()),
							samples: vec![
								TrunSample {
									composition_time_offset: Some(2000),
									duration: None,
									flags: None,
									size: Some(2232),
								},
								TrunSample {
									composition_time_offset: Some(2000),
									duration: None,
									flags: None,
									size: Some(238),
								},
								TrunSample {
									composition_time_offset: Some(2000),
									duration: None,
									flags: None,
									size: Some(19348),
								},
								TrunSample {
									composition_time_offset: Some(2000),
									duration: None,
									flags: None,
									size: Some(12481),
								},
								TrunSample {
									composition_time_offset: Some(7000),
									duration: None,
									flags: None,
									size: Some(30195),
								},
								TrunSample {
									composition_time_offset: Some(3000),
									duration: None,
									flags: None,
									size: Some(2200),
								},
								TrunSample {
									composition_time_offset: Some(0),
									duration: None,
									flags: None,
									size: Some(855),
								},
								TrunSample {
									composition_time_offset: Some(0),
									duration: None,
									flags: None,
									size: Some(820),
								},
								TrunSample {
									composition_time_offset: Some(1000),
									duration: None,
									flags: None,
									size: Some(991),
								},
								TrunSample {
									composition_time_offset: Some(1000),
									duration: None,
									flags: None,
									size: Some(747),
								},
								TrunSample {
									composition_time_offset: Some(2000),
									duration: None,
									flags: None,
									size: Some(9309),
								},
								TrunSample {
									composition_time_offset: Some(3000),
									duration: None,
									flags: None,
									size: Some(2475),
								},
								TrunSample {
									composition_time_offset: Some(1000),
									duration: None,
									flags: None,
									size: Some(1001),
								},
								TrunSample {
									composition_time_offset: Some(3000),
									duration: None,
									flags: None,
									size: Some(3175),
								},
								TrunSample {
									composition_time_offset: Some(1000),
									duration: None,
									flags: None,
									size: Some(698),
								},
							],
						}),
						sbgp: None,
						subs: None,
						unknown: vec![],
					},
					Traf {
						header: BoxHeader { box_type: *b"traf" },
						tfhd: Tfhd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfhd" },
								version: 0,
								flags: Tfhd::DEFAULT_BASE_IS_MOOF_FLAG
									| Tfhd::DEFAULT_SAMPLE_FLAGS_FLAG | Tfhd::DEFAULT_SAMPLE_SIZE_FLAG
									| Tfhd::DEFAULT_SAMPLE_DURATION_FLAG,
							},
							track_id: 2,
							base_data_offset: None,
							sample_description_index: None,
							default_sample_duration: Some(1024),
							default_sample_size: Some(24),
							default_sample_flags: Some(0x2000000.into()),
						},
						tfdt: Some(Tfdt {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfdt" },
								version: 1,
								flags: 0,
							},
							base_media_decode_time: 0,
						}),
						trun: Some(Trun {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"trun" },
								version: 0,
								flags: Trun::FLAG_DATA_OFFSET | Trun::FLAG_SAMPLE_SIZE,
							},
							data_offset: Some(87121),
							first_sample_flags: None,
							samples: vec![
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(24),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(8),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(303),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(630),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(619),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(621),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(631),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(624),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(610),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(446),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(419),
								},
								TrunSample {
									composition_time_offset: None,
									duration: None,
									flags: None,
									size: Some(417),
								},
							],
						}),
						sbgp: None,
						subs: None,
						unknown: vec![],
					},
				],
				unknown: vec![],
			}
		);
	}

	// mdat
	{
		let mdat = boxes[3].as_mdat().expect("mdat");
		assert_eq!(mdat.data.len(), 1);
		assert_eq!(mdat.data[0].len(), 92125 - 8); // 8 is mdat header size
	}
}

#[test]
fn test_mux_avc_aac() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
	let data = std::fs::read(dir.join("avc_aac_fragmented.mp4").to_str().unwrap()).unwrap();

	let mut boxes = Vec::new();
	let mut reader = io::Cursor::new(data.into());

	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		boxes.push(box_);
	}

	let mut writer = io::Cursor::new(Vec::new());
	for box_ in &boxes {
		box_.mux(&mut writer).unwrap();
	}

	let data = Bytes::from(writer.into_inner());
	let mut reader = io::Cursor::new(data.clone());

	let mut new_boxes = Vec::new();
	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		new_boxes.push(box_);
	}

	assert_eq!(boxes, new_boxes);

	// Pipe into ffprobe to check the output is valid.
	let mut ffprobe = Command::new("ffprobe")
		.arg("-v")
		.arg("error")
		.arg("-show_format")
		.arg("-show_streams")
		.arg("-print_format")
		.arg("json")
		.arg("-")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.unwrap();

	ffprobe.stdin.as_mut().unwrap().write_all(&data).expect("write to stdin");

	let output = ffprobe.wait_with_output().unwrap();
	assert!(output.status.success());

	// Check the output is valid.
	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	assert_eq!(json["format"]["format_name"], "mov,mp4,m4a,3gp,3g2,mj2");
	assert_eq!(json["format"]["nb_streams"], 2);
	assert_eq!(json["format"]["probe_score"], 100);

	assert_eq!(json["streams"][0]["codec_name"], "h264");
	assert_eq!(json["streams"][0]["codec_type"], "video");
	assert_eq!(json["streams"][0]["avg_frame_rate"], "60/1");
	assert_eq!(json["streams"][0]["r_frame_rate"], "60/1");
	assert_eq!(json["streams"][0]["height"], 2160);
	assert_eq!(json["streams"][0]["width"], 3840);

	assert_eq!(json["streams"][1]["codec_name"], "aac");
	assert_eq!(json["streams"][1]["codec_type"], "audio");
	assert_eq!(json["streams"][1]["sample_rate"], "48000");
	assert_eq!(json["streams"][1]["channels"], 2);
}

#[test]
fn test_demux_av1_aac() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

	let data = std::fs::read(dir.join("av1_aac_fragmented.mp4").to_str().unwrap()).unwrap();

	let mut boxes = Vec::new();
	let mut reader = io::Cursor::new(data.into());

	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		boxes.push(box_);
	}

	// The values we assert against are taken from `https://mlynoteka.mlyn.org/mp4parser/`
	// We assume this to be a correct implementation of the MP4 format.

	// ftyp
	{
		let ftyp = boxes[0].as_ftyp().expect("ftyp");
		assert_eq!(
			ftyp,
			&Ftyp {
				header: BoxHeader { box_type: *b"ftyp" },
				major_brand: FourCC::Iso5,
				minor_version: 512,
				compatible_brands: vec![FourCC::Iso5, FourCC::Iso6, FourCC::Av01, FourCC::Mp41],
			}
		);
	}

	// moov
	{
		let moov = boxes[1].as_moov().expect("moov");
		assert_eq!(
			moov.mvhd,
			Mvhd {
				header: FullBoxHeader {
					header: BoxHeader { box_type: *b"mvhd" },
					version: 0,
					flags: 0,
				},
				creation_time: 0,
				modification_time: 0,
				timescale: 1000,
				duration: 0,
				rate: FixedI32::from_num(1),
				volume: 1.into(),
				matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
				next_track_id: 2,
				reserved: 0,
				pre_defined: [0; 6],
				reserved2: [0; 2],
			}
		);

		// video track
		{
			let video_trak = &moov.traks[0];
			assert_eq!(
				video_trak.tkhd,
				Tkhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"tkhd" },
						version: 0,
						flags: 3,
					},
					creation_time: 0,
					modification_time: 0,
					track_id: 1,
					reserved: 0,
					duration: 0,
					reserved2: [0; 2],
					layer: 0,
					alternate_group: 0,
					volume: 0.into(),
					reserved3: 0,
					matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
					width: FixedI32::from_num(480),
					height: FixedI32::from_num(852),
				}
			);

			assert_eq!(
				video_trak.edts,
				Some(Edts {
					header: BoxHeader { box_type: *b"edts" },
					elst: Some(Elst {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"elst" },
							version: 0,
							flags: 0,
						},
						entries: vec![ElstEntry {
							segment_duration: 0,
							media_time: 0,
							media_rate_integer: 1,
							media_rate_fraction: 0,
						},],
					}),
					unknown: vec![],
				})
			);

			assert_eq!(
				video_trak.mdia.mdhd,
				Mdhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mdhd" },
						version: 0,
						flags: 0,
					},
					creation_time: 0,
					modification_time: 0,
					timescale: 15360,
					duration: 0,
					language: 21956,
					pre_defined: 0,
				}
			);

			assert_eq!(
				video_trak.mdia.hdlr,
				Hdlr {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"hdlr" },
						version: 0,
						flags: 0,
					},
					handler_type: HandlerType::Vide,
					reserved: [0; 3],
					name: "Core Media Video".into(),
					pre_defined: 0,
				}
			);

			assert_eq!(
				video_trak.mdia.minf.vmhd,
				Some(Vmhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"vmhd" },
						version: 0,
						flags: 1,
					},
					graphics_mode: 0,
					opcolor: [0, 0, 0],
				})
			);

			assert_eq!(
				video_trak.mdia.minf.dinf,
				Dinf {
					header: BoxHeader { box_type: *b"dinf" },
					unknown: vec![],
					dref: Dref {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"dref" },
							version: 0,
							flags: 0,
						},
						entries: vec![DynBox::Url(Url {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"url " },
								version: 0,
								flags: 1,
							},
							location: None,
						})],
					},
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsd,
				Stsd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsd" },
						version: 0,
						flags: 0,
					},
					entries: vec![DynBox::Av01(Av01 {
						header: BoxHeader { box_type: *b"av01" },
						visual_sample_entry: SampleEntry::<VisualSampleEntry> {
							reserved: [0; 6],
							data_reference_index: 1,
							extension: VisualSampleEntry {
								clap: None,
								colr: Some(Colr {
									header: BoxHeader { box_type: *b"colr" },
									color_type: ColorType::Nclx {
										color_primaries: 1,
										matrix_coefficients: 1,
										transfer_characteristics: 1,
										full_range_flag: false,
									},
								}),
								compressorname: [
									22, 76, 97, 118, 99, 54, 48, 46, 57, 46, 49, 48, 48, 32, 108, 105, 98, 115, 118, 116,
									97, 118, 49, 0, 0, 0, 0, 0, 0, 0, 0, 0
								],
								depth: 24,
								frame_count: 1,
								width: 480,
								height: 852,
								horizresolution: 4718592,
								vertresolution: 4718592,
								pre_defined2: [0; 3],
								pre_defined: 0,
								pre_defined3: -1,
								reserved: 0,
								reserved2: 0,
								pasp: None,
							}
						},
						av1c: Av1C {
							header: BoxHeader { box_type: *b"av1C" },
							av1_config: AV1CodecConfigurationRecord {
								marker: true,
								version: 1,
								seq_profile: 0,
								seq_level_idx_0: 4,
								seq_tier_0: false,
								high_bitdepth: false,
								twelve_bit: false,
								monochrome: false,
								chroma_subsampling_x: true,
								chroma_subsampling_y: true,
								chroma_sample_position: 1,
								initial_presentation_delay_minus_one: None,
								config_obu: b"\n\x0e\0\0\0$O\x7fS\0\xbe\x04\x04\x04\x04\x90".to_vec().into(),
							},
						},
						btrt: None,
						// This box is not defined in the spec, so we are not parsing it.
						unknown: vec![DynBox::Unknown((BoxHeader { box_type: *b"fiel" }, b"\x01\0".to_vec().into()))],
					})],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stts,
				Stts {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stts" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsc,
				Stsc {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsc" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsz,
				Some(Stsz {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsz" },
						version: 0,
						flags: 0,
					},
					sample_size: 0,
					samples: vec![],
				})
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stco,
				Stco {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stco" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(video_trak.mdia.minf.stbl.co64, None);

			assert_eq!(video_trak.mdia.minf.stbl.ctts, None);

			assert_eq!(video_trak.mdia.minf.stbl.padb, None);

			assert_eq!(video_trak.mdia.minf.stbl.sbgp, None);

			assert_eq!(video_trak.mdia.minf.stbl.sdtp, None);

			assert_eq!(video_trak.mdia.minf.stbl.stdp, None);

			assert_eq!(video_trak.mdia.minf.stbl.stsh, None);

			assert_eq!(video_trak.mdia.minf.stbl.stss, None);

			assert_eq!(video_trak.mdia.minf.stbl.stz2, None);

			assert_eq!(video_trak.mdia.minf.stbl.subs, None);
		}

		// audio track
		{
			let audio_trak = &moov.traks[1];
			assert_eq!(
				audio_trak.tkhd,
				Tkhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"tkhd" },
						version: 0,
						flags: 3,
					},
					creation_time: 0,
					modification_time: 0,
					track_id: 2,
					duration: 0,
					layer: 0,
					alternate_group: 1,
					volume: 1.into(),
					matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
					width: FixedI32::from_bits(0),
					height: FixedI32::from_bits(0),
					reserved: 0,
					reserved2: [0; 2],
					reserved3: 0,
				}
			);

			assert_eq!(
				audio_trak.edts,
				Some(Edts {
					header: BoxHeader { box_type: *b"edts" },
					elst: Some(Elst {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"elst" },
							version: 0,
							flags: 0,
						},
						entries: vec![ElstEntry {
							media_rate_fraction: 0,
							media_time: 1024,
							media_rate_integer: 1,
							segment_duration: 0,
						}],
					}),
					unknown: vec![],
				})
			);

			assert_eq!(
				audio_trak.mdia.mdhd,
				Mdhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mdhd" },
						version: 0,
						flags: 0,
					},
					creation_time: 0,
					modification_time: 0,
					timescale: 44100,
					duration: 0,
					language: 21956,
					pre_defined: 0,
				}
			);

			assert_eq!(
				audio_trak.mdia.hdlr,
				Hdlr {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"hdlr" },
						version: 0,
						flags: 0,
					},
					handler_type: (*b"soun").into(),
					name: "Core Media Audio".to_string(),
					pre_defined: 0,
					reserved: [0; 3],
				}
			);

			assert_eq!(
				audio_trak.mdia.minf,
				Minf {
					header: BoxHeader { box_type: *b"minf" },
					smhd: Some(Smhd {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"smhd" },
							version: 0,
							flags: 0,
						},
						balance: 0.into(),
						reserved: 0,
					}),
					dinf: Dinf {
						header: BoxHeader { box_type: *b"dinf" },
						dref: Dref {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"dref" },
								version: 0,
								flags: 0,
							},
							entries: vec![DynBox::Url(Url {
								header: FullBoxHeader {
									header: BoxHeader { box_type: *b"url " },
									version: 0,
									flags: 1,
								},
								location: None,
							})],
						},
						unknown: vec![],
					},
					stbl: Stbl {
						header: BoxHeader { box_type: *b"stbl" },
						stsd: Stsd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsd" },
								version: 0,
								flags: 0,
							},
							entries: vec![DynBox::Mp4a(Mp4a {
								header: BoxHeader { box_type: *b"mp4a" },
								audio_sample_entry: SampleEntry::<AudioSampleEntry> {
									data_reference_index: 1,
									reserved: [0; 6],
									extension: AudioSampleEntry {
										channel_count: 1,
										pre_defined: 0,
										reserved2: 0,
										sample_size: 16,
										reserved: [0; 2],
										sample_rate: 44100,
									},
								},
								esds: Esds {
									header: FullBoxHeader {
										header: BoxHeader { box_type: *b"esds" },
										version: 0,
										flags: 0,
									},
									es_descriptor: EsDescriptor {
										header: DescriptorHeader { tag: 3.into() },
										es_id: 2,
										depends_on_es_id: None,
										ocr_es_id: None,
										stream_priority: 0,
										url: None,
										decoder_config: Some(DecoderConfigDescriptor {
											header: DescriptorHeader { tag: 4.into() },
											avg_bitrate: 69000,
											buffer_size_db: 0,
											max_bitrate: 69000,
											reserved: 1,
											stream_type: 5,
											object_type_indication: 64,
											up_stream: false,
											decoder_specific_info: Some(DecoderSpecificInfoDescriptor {
												header: DescriptorHeader { tag: 5.into() },
												data: Bytes::from_static(b"\x12\x08V\xe5\0"),
											}),
											unknown: vec![],
										}),
										sl_config: Some(SLConfigDescriptor {
											header: DescriptorHeader { tag: 6.into() },
											predefined: 2,
											data: Bytes::new(),
										}),
										unknown: vec![],
									},
									unknown: vec![],
								},
								btrt: Some(Btrt {
									header: BoxHeader { box_type: *b"btrt" },
									buffer_size_db: 0,
									max_bitrate: 69000,
									avg_bitrate: 69000,
								}),
								unknown: vec![],
							})],
						},
						co64: None,
						ctts: None,
						padb: None,
						sbgp: None,
						sdtp: None,
						stdp: None,
						stco: Stco {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stco" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stsc: Stsc {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsc" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stsh: None,
						stss: None,
						stsz: Some(Stsz {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsz" },
								version: 0,
								flags: 0,
							},
							sample_size: 0,
							samples: vec![],
						}),
						stts: Stts {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stts" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stz2: None,
						subs: None,
						unknown: vec![],
					},
					hmhd: None,
					nmhd: None,
					vmhd: None,
					unknown: vec![],
				}
			);
		}

		// mvex
		assert_eq!(
			moov.mvex,
			Some(Mvex {
				header: BoxHeader { box_type: *b"mvex" },
				mehd: None,
				trex: vec![
					Trex {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"trex" },
							version: 0,
							flags: 0,
						},
						track_id: 1,
						default_sample_description_index: 1,
						default_sample_duration: 0,
						default_sample_size: 0,
						default_sample_flags: 0,
					},
					Trex {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"trex" },
							version: 0,
							flags: 0,
						},
						track_id: 2,
						default_sample_description_index: 1,
						default_sample_duration: 0,
						default_sample_size: 0,
						default_sample_flags: 0,
					},
				],
				unknown: vec![],
			})
		)
	}

	// moof
	{
		let moof = boxes[2].as_moof().expect("moof");
		assert_eq!(
			moof,
			&Moof {
				header: BoxHeader { box_type: *b"moof" },
				mfhd: Mfhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mfhd" },
						version: 0,
						flags: 0,
					},
					sequence_number: 1,
				},
				traf: vec![
					Traf {
						header: BoxHeader { box_type: *b"traf" },
						tfhd: Tfhd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfhd" },
								version: 0,
								flags: Tfhd::DEFAULT_BASE_IS_MOOF_FLAG
									| Tfhd::DEFAULT_SAMPLE_FLAGS_FLAG | Tfhd::DEFAULT_SAMPLE_SIZE_FLAG
									| Tfhd::DEFAULT_SAMPLE_DURATION_FLAG,
							},
							track_id: 1,
							base_data_offset: None,
							sample_description_index: None,
							default_sample_duration: Some(512),
							default_sample_size: Some(26336),
							default_sample_flags: Some(16842752.into()),
						},
						tfdt: Some(Tfdt {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfdt" },
								version: 1,
								flags: 0,
							},
							base_media_decode_time: 0,
						}),
						trun: Some(Trun {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"trun" },
								version: 0,
								flags: Trun::FLAG_FIRST_SAMPLE_FLAGS | Trun::FLAG_DATA_OFFSET
							},
							data_offset: Some(188),
							first_sample_flags: Some(33554432.into()),
							samples: vec![TrunSample {
								composition_time_offset: None,
								duration: None,
								flags: None,
								size: None,
							},],
						}),
						sbgp: None,
						subs: None,
						unknown: vec![],
					},
					Traf {
						header: BoxHeader { box_type: *b"traf" },
						tfhd: Tfhd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfhd" },
								version: 0,
								flags: Tfhd::DEFAULT_BASE_IS_MOOF_FLAG
									| Tfhd::DEFAULT_SAMPLE_FLAGS_FLAG | Tfhd::DEFAULT_SAMPLE_SIZE_FLAG
									| Tfhd::DEFAULT_SAMPLE_DURATION_FLAG,
							},
							track_id: 2,
							base_data_offset: None,
							sample_description_index: None,
							default_sample_duration: Some(1024),
							default_sample_size: Some(234),
							default_sample_flags: Some(0x2000000.into()),
						},
						tfdt: Some(Tfdt {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfdt" },
								version: 1,
								flags: 0,
							},
							base_media_decode_time: 0,
						}),
						trun: Some(Trun {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"trun" },
								version: 0,
								flags: Trun::FLAG_DATA_OFFSET
							},
							data_offset: Some(26524),
							first_sample_flags: None,
							samples: vec![TrunSample {
								composition_time_offset: None,
								duration: None,
								flags: None,
								size: None,
							},],
						}),
						sbgp: None,
						subs: None,
						unknown: vec![],
					},
				],
				unknown: vec![],
			}
		);
	}

	// mdat
	{
		let mdat = boxes[3].as_mdat().expect("mdat");
		assert_eq!(mdat.data.len(), 1);
		assert_eq!(mdat.data[0].len(), 26578 - 8); // 8 is mdat header size
	}
}

#[test]
fn test_demux_hevc_aac() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

	let data = std::fs::read(dir.join("hevc_aac_fragmented.mp4").to_str().unwrap()).unwrap();

	let mut boxes = Vec::new();
	let mut reader = io::Cursor::new(data.into());

	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		boxes.push(box_);
	}

	// The values we assert against are taken from `https://mlynoteka.mlyn.org/mp4parser/`
	// We assume this to be a correct implementation of the MP4 format.

	// ftyp
	{
		let ftyp = boxes[0].as_ftyp().expect("ftyp");
		assert_eq!(
			ftyp,
			&Ftyp {
				header: BoxHeader { box_type: *b"ftyp" },
				major_brand: FourCC::Iso5,
				minor_version: 512,
				compatible_brands: vec![FourCC::Iso5, FourCC::Iso6, FourCC::Mp41],
			}
		);
	}

	// moov
	{
		let moov = boxes[1].as_moov().expect("moov");
		assert_eq!(
			moov.mvhd,
			Mvhd {
				header: FullBoxHeader {
					header: BoxHeader { box_type: *b"mvhd" },
					version: 0,
					flags: 0,
				},
				creation_time: 0,
				modification_time: 0,
				timescale: 1000,
				duration: 0,
				rate: FixedI32::from_num(1),
				volume: 1.into(),
				matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
				next_track_id: 2,
				reserved: 0,
				pre_defined: [0; 6],
				reserved2: [0; 2],
			}
		);

		// video track
		{
			let video_trak = &moov.traks[0];
			assert_eq!(
				video_trak.tkhd,
				Tkhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"tkhd" },
						version: 0,
						flags: 3,
					},
					creation_time: 0,
					modification_time: 0,
					track_id: 1,
					reserved: 0,
					duration: 0,
					reserved2: [0; 2],
					layer: 0,
					alternate_group: 0,
					volume: 0.into(),
					reserved3: 0,
					matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
					width: FixedI32::from_num(3840),
					height: FixedI32::from_num(2160),
				}
			);

			assert_eq!(
				video_trak.edts,
				Some(Edts {
					header: BoxHeader { box_type: *b"edts" },
					elst: Some(Elst {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"elst" },
							version: 0,
							flags: 0,
						},
						entries: vec![ElstEntry {
							segment_duration: 0,
							media_time: 512,
							media_rate_integer: 1,
							media_rate_fraction: 0,
						},],
					}),
					unknown: vec![],
				})
			);

			assert_eq!(
				video_trak.mdia.mdhd,
				Mdhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mdhd" },
						version: 0,
						flags: 0,
					},
					creation_time: 0,
					modification_time: 0,
					timescale: 15360,
					duration: 0,
					language: 21956,
					pre_defined: 0,
				}
			);

			assert_eq!(
				video_trak.mdia.hdlr,
				Hdlr {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"hdlr" },
						version: 0,
						flags: 0,
					},
					handler_type: HandlerType::Vide,
					reserved: [0; 3],
					name: "GPAC ISO Video Handler".into(),
					pre_defined: 0,
				}
			);

			assert_eq!(
				video_trak.mdia.minf.vmhd,
				Some(Vmhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"vmhd" },
						version: 0,
						flags: 1,
					},
					graphics_mode: 0,
					opcolor: [0, 0, 0],
				})
			);

			assert_eq!(
				video_trak.mdia.minf.dinf,
				Dinf {
					header: BoxHeader { box_type: *b"dinf" },
					unknown: vec![],
					dref: Dref {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"dref" },
							version: 0,
							flags: 0,
						},
						entries: vec![DynBox::Url(Url {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"url " },
								version: 0,
								flags: 1,
							},
							location: None,
						})],
					},
				}
			);

			assert_eq!(
                video_trak.mdia.minf.stbl.stsd,
                Stsd {
                    header: FullBoxHeader {
                        header: BoxHeader { box_type: *b"stsd" },
                        version: 0,
                        flags: 0,
                    },
                    entries: vec![
                        DynBox::Hev1(Hev1 {
                            header: BoxHeader { box_type: *b"hev1" },
                            visual_sample_entry: SampleEntry {
                                reserved: [0, 0, 0, 0, 0, 0],
                                data_reference_index: 1,
                                extension: VisualSampleEntry {
                                    pre_defined: 0,
                                    reserved: 0,
                                    pre_defined2: [0, 0, 0],
                                    width: 3840,
                                    height: 2160,
                                    horizresolution: 4718592,
                                    vertresolution: 4718592,
                                    reserved2: 0,
                                    frame_count: 1,
                                    compressorname: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                                    depth: 24,
                                    pre_defined3: -1,
                                    clap: None,
                                    colr: None,
                                    pasp: Some(Pasp {
                                        header: BoxHeader { box_type: *b"pasp" },
                                        h_spacing: 1,
                                        v_spacing: 1
                                    })
                                }
                            },
                            hvcc: HvcC {
                                header: BoxHeader { box_type: *b"hvcC" },
                                hevc_config: HEVCDecoderConfigurationRecord {
                                    configuration_version: 1,
                                    general_profile_space: 0,
                                    general_tier_flag: false,
                                    general_profile_idc: 1,
                                    general_profile_compatibility_flags: 96,
                                    general_constraint_indicator_flags: 144,
                                    general_level_idc: 153,
                                    min_spatial_segmentation_idc: 0,
                                    parallelism_type: 0,
                                    chroma_format_idc: 1,
                                    bit_depth_luma_minus8: 0,
                                    bit_depth_chroma_minus8: 0,
                                    avg_frame_rate: 0,
                                    constant_frame_rate: 0,
                                    num_temporal_layers: 1,
                                    temporal_id_nested: true,
                                    length_size_minus_one: 3,
                                    arrays: vec![
                                        NaluArray {
                                            array_completeness: false,
                                            nal_unit_type: NaluType::Vps,
                                            nalus: vec![b"@\x01\x0c\x01\xff\xff\x01`\0\0\x03\0\x90\0\0\x03\0\0\x03\0\x99\x95\x98\t".to_vec().into()]
                                        },
                                        NaluArray {
                                            array_completeness: false,
                                            nal_unit_type: NaluType::Sps,
                                            nalus: vec![b"B\x01\x01\x01`\0\0\x03\0\x90\0\0\x03\0\0\x03\0\x99\xa0\x01\xe0 \x02\x1cYef\x92L\xaf\x01h\x08\0\0\x03\0\x08\0\0\x03\x01\xe0@".to_vec().into()]
                                        },
                                        NaluArray {
                                            array_completeness: false,
                                            nal_unit_type: NaluType::Pps,
                                            nalus: vec![b"D\x01\xc1r\xb4\"@".to_vec().into()],
                                        },
                                        NaluArray {
                                            array_completeness: false,
                                            nal_unit_type: NaluType::Unknown(39),
                                            nalus: vec![b"N\x01\x05\xff\xff\xff\xff\xff\xff\xff\xff\xf5,\xa2\xde\t\xb5\x17G\xdb\xbbU\xa4\xfe\x7f\xc2\xfcNx265 (build 199) - 3.5+1-f0c1022b6:[Linux][GCC 11.2.0][64 bit] 8bit+10bit+12bit - H.265/HEVC codec - Copyright 2013-2018 (c) Multicoreware, Inc - http://x265.org - options: cpuid=1111039 frame-threads=6 no-wpp no-pmode no-pme no-psnr no-ssim log-level=2 bitdepth=8 input-csp=1 fps=60/1 input-res=3840x2160 interlace=0 total-frames=0 level-idc=0 high-tier=1 uhd-bd=0 ref=3 no-allow-non-conformance no-repeat-headers annexb no-aud no-hrd info hash=0 no-temporal-layers open-gop min-keyint=25 keyint=250 gop-lookahead=0 bframes=4 b-adapt=2 b-pyramid bframe-bias=0 rc-lookahead=20 lookahead-slices=0 scenecut=40 hist-scenecut=0 radl=0 no-splice no-intra-refresh ctu=64 min-cu-size=8 no-rect no-amp max-tu-size=32 tu-inter-depth=1 tu-intra-depth=1 limit-tu=0 rdoq-level=0 dynamic-rd=0.00 no-ssim-rd signhide no-tskip nr-intra=0 nr-inter=0 no-constrained-intra strong-intra-smoothing max-merge=3 limit-refs=1 no-limit-modes me=1 subme=2 merange=57 temporal-mvp no-frame-dup no-hme weightp no-weightb no-analyze-src-pics deblock=0:0 sao no-sao-non-deblock rd=3 selective-sao=4 early-skip rskip no-fast-intra no-tskip-fast no-cu-lossless b-intra no-splitrd-skip rdpenalty=0 psy-rd=2.00 psy-rdoq=0.00 no-rd-refine no-lossless cbqpoffs=0 crqpoffs=0 rc=crf crf=28.0 qcomp=0.60 qpstep=4 stats-write=0 stats-read=0 ipratio=1.40 pbratio=1.30 aq-mode=2 aq-strength=1.00 cutree zone-count=0 no-strict-cbr qg-size=32 no-rc-grain qpmax=69 qpmin=0 no-const-vbv sar=1 overscan=0 videoformat=5 range=0 colorprim=2 transfer=2 colormatrix=2 chromaloc=0 display-window=0 cll=0,0 min-luma=0 max-luma=255 log2-max-poc-lsb=8 vui-timing-info vui-hrd-info slices=1 no-opt-qp-pps no-opt-ref-list-length-pps no-multi-pass-opt-rps scenecut-bias=0.05 hist-threshold=0.03 no-opt-cu-delta-qp no-aq-motion no-hdr10 no-hdr10-opt no-dhdr10-opt no-idr-recovery-sei analysis-reuse-level=0 analysis-save-reuse-level=0 analysis-load-reuse-level=0 scale-factor=0 refine-intra=0 refine-inter=0 refine-mv=1 refine-ctu-distortion=0 no-limit-sao ctu-info=0 no-lowpass-dct refine-analysis-type=0 copy-pic=1 max-ausize-factor=1.0 no-dynamic-refine no-single-sei no-hevc-aq no-svt no-field qp-adaptation-range=1.00 scenecut-aware-qp=0conformance-window-offsets right=0 bottom=0 decoder-max-rate=0 no-vbv-live-multi-pass\x80".to_vec().into()]
                                        }
                                    ]
                                }
                            },
                            btrt: None,
                            unknown: vec![
                                DynBox::Unknown((BoxHeader { box_type: *b"fiel" }, b"\x01\0".to_vec().into()))
                            ],
                        }),
                    ],
                }
            );

			assert_eq!(
				video_trak.mdia.minf.stbl.stts,
				Stts {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stts" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsc,
				Stsc {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsc" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stsz,
				Some(Stsz {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stsz" },
						version: 0,
						flags: 0,
					},
					sample_size: 0,
					samples: vec![],
				})
			);

			assert_eq!(
				video_trak.mdia.minf.stbl.stco,
				Stco {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"stco" },
						version: 0,
						flags: 0,
					},
					entries: vec![],
				}
			);

			assert_eq!(video_trak.mdia.minf.stbl.co64, None);

			assert_eq!(video_trak.mdia.minf.stbl.ctts, None);

			assert_eq!(video_trak.mdia.minf.stbl.padb, None);

			assert_eq!(video_trak.mdia.minf.stbl.sbgp, None);

			assert_eq!(video_trak.mdia.minf.stbl.sdtp, None);

			assert_eq!(video_trak.mdia.minf.stbl.stdp, None);

			assert_eq!(video_trak.mdia.minf.stbl.stsh, None);

			assert_eq!(video_trak.mdia.minf.stbl.stss, None);

			assert_eq!(video_trak.mdia.minf.stbl.stz2, None);

			assert_eq!(video_trak.mdia.minf.stbl.subs, None);
		}

		// audio track
		{
			let audio_trak = &moov.traks[1];
			assert_eq!(
				audio_trak.tkhd,
				Tkhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"tkhd" },
						version: 0,
						flags: 3,
					},
					creation_time: 0,
					modification_time: 0,
					track_id: 2,
					duration: 0,
					layer: 0,
					alternate_group: 1,
					volume: 1.into(),
					matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
					width: FixedI32::from_bits(0),
					height: FixedI32::from_bits(0),
					reserved: 0,
					reserved2: [0; 2],
					reserved3: 0,
				}
			);

			assert_eq!(
				audio_trak.edts,
				Some(Edts {
					header: BoxHeader { box_type: *b"edts" },
					elst: Some(Elst {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"elst" },
							version: 0,
							flags: 0,
						},
						entries: vec![ElstEntry {
							media_rate_fraction: 0,
							media_time: 1024,
							media_rate_integer: 1,
							segment_duration: 0,
						}],
					}),
					unknown: vec![],
				})
			);

			assert_eq!(
				audio_trak.mdia.mdhd,
				Mdhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mdhd" },
						version: 0,
						flags: 0,
					},
					creation_time: 0,
					modification_time: 0,
					timescale: 48000,
					duration: 0,
					language: 21956,
					pre_defined: 0,
				}
			);

			assert_eq!(
				audio_trak.mdia.hdlr,
				Hdlr {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"hdlr" },
						version: 0,
						flags: 0,
					},
					handler_type: (*b"soun").into(),
					name: "GPAC ISO Audio Handler".to_string(),
					pre_defined: 0,
					reserved: [0; 3],
				}
			);

			assert_eq!(
				audio_trak.mdia.minf,
				Minf {
					header: BoxHeader { box_type: *b"minf" },
					smhd: Some(Smhd {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"smhd" },
							version: 0,
							flags: 0,
						},
						balance: 0.into(),
						reserved: 0,
					}),
					dinf: Dinf {
						header: BoxHeader { box_type: *b"dinf" },
						dref: Dref {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"dref" },
								version: 0,
								flags: 0,
							},
							entries: vec![DynBox::Url(Url {
								header: FullBoxHeader {
									header: BoxHeader { box_type: *b"url " },
									version: 0,
									flags: 1,
								},
								location: None,
							})],
						},
						unknown: vec![],
					},
					stbl: Stbl {
						header: BoxHeader { box_type: *b"stbl" },
						stsd: Stsd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsd" },
								version: 0,
								flags: 0,
							},
							entries: vec![DynBox::Mp4a(Mp4a {
								header: BoxHeader { box_type: *b"mp4a" },
								audio_sample_entry: SampleEntry::<AudioSampleEntry> {
									data_reference_index: 1,
									reserved: [0; 6],
									extension: AudioSampleEntry {
										channel_count: 2,
										pre_defined: 0,
										reserved2: 0,
										sample_size: 16,
										reserved: [0; 2],
										sample_rate: 48000,
									},
								},
								esds: Esds {
									header: FullBoxHeader {
										header: BoxHeader { box_type: *b"esds" },
										version: 0,
										flags: 0,
									},
									es_descriptor: EsDescriptor {
										header: DescriptorHeader { tag: 3.into() },
										es_id: 2,
										depends_on_es_id: None,
										ocr_es_id: None,
										stream_priority: 0,
										url: None,
										decoder_config: Some(DecoderConfigDescriptor {
											header: DescriptorHeader { tag: 4.into() },
											avg_bitrate: 128000,
											buffer_size_db: 0,
											max_bitrate: 128000,
											reserved: 1,
											stream_type: 5,
											object_type_indication: 64,
											up_stream: false,
											decoder_specific_info: Some(DecoderSpecificInfoDescriptor {
												header: DescriptorHeader { tag: 5.into() },
												data: Bytes::from_static(b"\x11\x90V\xe5\0"),
											}),
											unknown: vec![],
										}),
										sl_config: Some(SLConfigDescriptor {
											header: DescriptorHeader { tag: 6.into() },
											predefined: 2,
											data: Bytes::new(),
										}),
										unknown: vec![],
									},
									unknown: vec![],
								},
								btrt: Some(Btrt {
									header: BoxHeader { box_type: *b"btrt" },
									buffer_size_db: 0,
									max_bitrate: 128000,
									avg_bitrate: 128000,
								}),
								unknown: vec![],
							})],
						},
						co64: None,
						ctts: None,
						padb: None,
						sbgp: None,
						sdtp: None,
						stdp: None,
						stco: Stco {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stco" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stsc: Stsc {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsc" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stsh: None,
						stss: None,
						stsz: Some(Stsz {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stsz" },
								version: 0,
								flags: 0,
							},
							sample_size: 0,
							samples: vec![],
						}),
						stts: Stts {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"stts" },
								version: 0,
								flags: 0,
							},
							entries: vec![],
						},
						stz2: None,
						subs: None,
						unknown: vec![],
					},
					hmhd: None,
					nmhd: None,
					vmhd: None,
					unknown: vec![],
				}
			);
		}

		// mvex
		assert_eq!(
			moov.mvex,
			Some(Mvex {
				header: BoxHeader { box_type: *b"mvex" },
				mehd: None,
				trex: vec![
					Trex {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"trex" },
							version: 0,
							flags: 0,
						},
						track_id: 1,
						default_sample_description_index: 1,
						default_sample_duration: 0,
						default_sample_size: 0,
						default_sample_flags: 0,
					},
					Trex {
						header: FullBoxHeader {
							header: BoxHeader { box_type: *b"trex" },
							version: 0,
							flags: 0,
						},
						track_id: 2,
						default_sample_description_index: 1,
						default_sample_duration: 0,
						default_sample_size: 0,
						default_sample_flags: 0,
					},
				],
				unknown: vec![],
			})
		)
	}

	// moof
	{
		let moof = boxes[2].as_moof().expect("moof");
		assert_eq!(
			moof,
			&Moof {
				header: BoxHeader { box_type: *b"moof" },
				mfhd: Mfhd {
					header: FullBoxHeader {
						header: BoxHeader { box_type: *b"mfhd" },
						version: 0,
						flags: 0,
					},
					sequence_number: 1,
				},
				traf: vec![
					Traf {
						header: BoxHeader { box_type: *b"traf" },
						tfhd: Tfhd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfhd" },
								version: 0,
								flags: Tfhd::DEFAULT_BASE_IS_MOOF_FLAG
									| Tfhd::DEFAULT_SAMPLE_FLAGS_FLAG | Tfhd::DEFAULT_SAMPLE_SIZE_FLAG
									| Tfhd::DEFAULT_SAMPLE_DURATION_FLAG,
							},
							track_id: 1,
							base_data_offset: None,
							sample_description_index: None,
							default_sample_duration: Some(256),
							default_sample_size: Some(1873),
							default_sample_flags: Some(0x1010000.into()),
						},
						tfdt: Some(Tfdt {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfdt" },
								version: 1,
								flags: 0,
							},
							base_media_decode_time: 0,
						}),
						trun: Some(Trun {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"trun" },
								version: 0,
								flags: Trun::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET
									| Trun::FLAG_FIRST_SAMPLE_FLAGS | Trun::FLAG_DATA_OFFSET
							},
							data_offset: Some(192),
							first_sample_flags: Some(33554432.into()),
							samples: vec![TrunSample {
								composition_time_offset: Some(512),
								duration: None,
								flags: None,
								size: None,
							},],
						}),
						sbgp: None,
						subs: None,
						unknown: vec![],
					},
					Traf {
						header: BoxHeader { box_type: *b"traf" },
						tfhd: Tfhd {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfhd" },
								version: 0,
								flags: Tfhd::DEFAULT_BASE_IS_MOOF_FLAG
									| Tfhd::DEFAULT_SAMPLE_FLAGS_FLAG | Tfhd::DEFAULT_SAMPLE_SIZE_FLAG
									| Tfhd::DEFAULT_SAMPLE_DURATION_FLAG,
							},
							track_id: 2,
							base_data_offset: None,
							sample_description_index: None,
							default_sample_duration: Some(1024),
							default_sample_size: Some(24),
							default_sample_flags: Some(0x2000000.into()),
						},
						tfdt: Some(Tfdt {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"tfdt" },
								version: 1,
								flags: 0,
							},
							base_media_decode_time: 0,
						}),
						trun: Some(Trun {
							header: FullBoxHeader {
								header: BoxHeader { box_type: *b"trun" },
								version: 0,
								flags: Trun::FLAG_DATA_OFFSET,
							},
							data_offset: Some(2065),
							first_sample_flags: None,
							samples: vec![TrunSample {
								composition_time_offset: None,
								duration: None,
								flags: None,
								size: None,
							},],
						}),
						sbgp: None,
						subs: None,
						unknown: vec![],
					},
				],
				unknown: vec![],
			}
		);
	}

	// mdat
	{
		let mdat = boxes[3].as_mdat().expect("mdat");
		assert_eq!(mdat.data.len(), 1);
		assert_eq!(mdat.data[0].len(), 1905 - 8); // 8 is mdat header size
	}
}

#[test]
fn test_mux_av1_aac() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
	let data = std::fs::read(dir.join("av1_aac_fragmented.mp4").to_str().unwrap()).unwrap();

	let mut boxes = Vec::new();
	let mut reader = io::Cursor::new(data.into());

	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		boxes.push(box_);
	}

	let mut writer = io::Cursor::new(Vec::new());
	for box_ in &boxes {
		box_.mux(&mut writer).unwrap();
	}

	let data = Bytes::from(writer.into_inner());
	let mut reader = io::Cursor::new(data.clone());

	let mut new_boxes = Vec::new();
	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		new_boxes.push(box_);
	}

	assert_eq!(boxes, new_boxes);

	// Pipe into ffprobe to check the output is valid.
	let mut ffprobe = Command::new("ffprobe")
		.arg("-v")
		.arg("error")
		.arg("-show_format")
		.arg("-show_streams")
		.arg("-print_format")
		.arg("json")
		.arg("-")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.unwrap();

	let _ = ffprobe.stdin.as_mut().unwrap().write_all(&data);

	let output = ffprobe.wait_with_output().unwrap();
	assert!(output.status.success());

	// Check the output is valid.
	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	assert_eq!(json["format"]["format_name"], "mov,mp4,m4a,3gp,3g2,mj2");
	assert_eq!(json["format"]["nb_streams"], 2);
	assert_eq!(json["format"]["probe_score"], 100);

	assert_eq!(json["streams"][0]["codec_name"], "av1");
	assert_eq!(json["streams"][0]["codec_type"], "video");
	assert_eq!(json["streams"][0]["avg_frame_rate"], "30/1");
	assert_eq!(json["streams"][0]["r_frame_rate"], "30/1");
	assert_eq!(json["streams"][0]["height"], 852);
	assert_eq!(json["streams"][0]["width"], 480);

	assert_eq!(json["streams"][1]["codec_name"], "aac");
	assert_eq!(json["streams"][1]["codec_type"], "audio");
	assert_eq!(json["streams"][1]["sample_rate"], "44100");
	assert_eq!(json["streams"][1]["channels"], 1);
}

#[test]
fn test_mux_hevc_aac() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
	let data = std::fs::read(dir.join("hevc_aac_fragmented.mp4").to_str().unwrap()).unwrap();

	let mut boxes = Vec::new();
	let mut reader = io::Cursor::new(data.into());

	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		boxes.push(box_);
	}

	let mut writer = io::Cursor::new(Vec::new());
	for box_ in &boxes {
		box_.mux(&mut writer).unwrap();
	}

	let data = Bytes::from(writer.into_inner());
	let mut reader = io::Cursor::new(data.clone());

	let mut new_boxes = Vec::new();
	while reader.has_remaining() {
		let box_ = DynBox::demux(&mut reader).unwrap();
		new_boxes.push(box_);
	}

	assert_eq!(boxes, new_boxes);

	// Pipe into ffprobe to check the output is valid.
	let mut ffprobe = Command::new("ffprobe")
		.arg("-v")
		.arg("error")
		.arg("-show_format")
		.arg("-show_streams")
		.arg("-print_format")
		.arg("json")
		.arg("-")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.unwrap();

	let _ = ffprobe.stdin.as_mut().unwrap().write_all(&data);

	let output = ffprobe.wait_with_output().unwrap();
	assert!(output.status.success());

	// Check the output is valid.
	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	assert_eq!(json["format"]["format_name"], "mov,mp4,m4a,3gp,3g2,mj2");
	assert_eq!(json["format"]["nb_streams"], 2);
	assert_eq!(json["format"]["probe_score"], 100);

	assert_eq!(json["streams"][0]["codec_name"], "hevc");
	assert_eq!(json["streams"][0]["codec_type"], "video");
	assert_eq!(json["streams"][0]["avg_frame_rate"], "60/1");
	assert_eq!(json["streams"][0]["r_frame_rate"], "60/1");
	assert_eq!(json["streams"][0]["height"], 2160);
	assert_eq!(json["streams"][0]["width"], 3840);

	assert_eq!(json["streams"][1]["codec_name"], "aac");
	assert_eq!(json["streams"][1]["codec_type"], "audio");
	assert_eq!(json["streams"][1]["sample_rate"], "48000");
	assert_eq!(json["streams"][1]["channels"], 2);
}
