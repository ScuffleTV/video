use std::fmt;
use std::str::FromStr;

use aac::AudioObjectType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
	/// https://developer.mozilla.org/en-US/docs/Web/Media/Formats/codecs_parameter
	Avc { profile: u8, constraint_set: u8, level: u8 },
	/// There is barely any documentation on this.
	/// http://hevcvideo.xp3.biz/html5_video.html
	Hevc {
		general_profile_space: u8,
		profile_compatibility: u32,
		profile: u8,
		level: u8,
		tier: bool,
		constraint_indicator: u64,
	},
	/// https://developer.mozilla.org/en-US/docs/Web/Media/Formats/codecs_parameter#av1
	Av1 {
		profile: u8,
		level: u8,
		tier: bool,
		depth: u8,
		monochrome: bool,
		sub_sampling_x: bool,
		sub_sampling_y: bool,
		color_primaries: u8,
		transfer_characteristics: u8,
		matrix_coefficients: u8,
		full_range_flag: bool,
	},
}

impl fmt::Display for VideoCodec {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			VideoCodec::Avc {
				profile,
				constraint_set,
				level,
			} => write!(f, "avc1.{:02x}{:02x}{:02x}", profile, constraint_set, level),
			VideoCodec::Hevc {
				general_profile_space,
				profile,
				level,
				tier,
				profile_compatibility,
				constraint_indicator,
			} => write!(
				f,
				"hev1.{}{:x}.{:x}.{}{:x}.{:x}",
				match general_profile_space {
					1 => "A",
					2 => "B",
					3 => "C",
					_ => "",
				},
				profile, // 1 or 2 chars (hex)
				profile_compatibility,
				if *tier { 'H' } else { 'L' },
				level, // 1 or 2 chars (hex)
				constraint_indicator,
			),
			VideoCodec::Av1 {
				profile,
				level,
				tier,
				depth,
				monochrome,
				sub_sampling_x,
				sub_sampling_y,
				color_primaries,
				transfer_characteristics,
				matrix_coefficients,
				full_range_flag,
			} => write!(
				f,
				"av01.{}.{}{}.{:02}.{}.{}{}{}.{:02}.{:02}.{:02}.{}",
				profile,
				level,
				if *tier { 'H' } else { 'M' },
				depth,
				if *monochrome { 1 } else { 0 },
				if *sub_sampling_x { 1 } else { 0 },
				if *sub_sampling_y { 1 } else { 0 },
				if *monochrome { 1 } else { 0 },
				color_primaries,
				transfer_characteristics,
				matrix_coefficients,
				if *full_range_flag { 1 } else { 0 },
			),
		}
	}
}

impl FromStr for VideoCodec {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let splits = s.split('.').collect::<Vec<_>>();
		if splits.is_empty() {
			return Err("invalid codec, empty string".into());
		}

		match splits[0] {
			"avc1" => {
				if splits.len() < 2 {
					return Err("invalid codec, missing profile".into());
				}

				let profile = u8::from_str_radix(&splits[1][..2], 16)
					.map_err(|e| format!("invalid codec, invalid profile: {}, {}", splits[1], e))?;
				let constraint_set = u8::from_str_radix(&splits[1][2..4], 16)
					.map_err(|e| format!("invalid codec, invalid constraint set: {}, {}", splits[1], e))?;
				let level = u8::from_str_radix(&splits[1][4..6], 16)
					.map_err(|e| format!("invalid codec, invalid level: {}, {}", splits[1], e))?;

				Ok(VideoCodec::Avc {
					profile,
					constraint_set,
					level,
				})
			}
			"hev1" => {
				if splits.len() < 6 {
					return Err("invalid codec, missing profile".into());
				}

				let general_profile_space = match splits[1] {
					"A" => 1,
					"B" => 2,
					"C" => 3,
					_ => {
						return Err(format!("invalid codec, invalid general profile space: {}", splits[1]));
					}
				};

				let profile = u8::from_str_radix(splits[2], 16)
					.map_err(|e| format!("invalid codec, invalid profile: {}, {}", splits[2], e))?;

				let profile_compatibility = u32::from_str_radix(splits[3], 16)
					.map_err(|e| format!("invalid codec, invalid profile compatibility: {}, {}", splits[3], e))?;

				let tier = match splits[4] {
					"H" => true,
					"L" => false,
					_ => return Err(format!("invalid codec, invalid tier: {}", splits[4])),
				};

				let level = u8::from_str_radix(splits[5], 16)
					.map_err(|e| format!("invalid codec, invalid level: {}, {}", splits[5], e))?;

				let constraint_indicator = u64::from_str_radix(splits[6], 16)
					.map_err(|e| format!("invalid codec, invalid constraint indicator: {}, {}", splits[6], e))?;

				Ok(VideoCodec::Hevc {
					general_profile_space,
					profile,
					level,
					tier,
					profile_compatibility,
					constraint_indicator,
				})
			}
			"av01" => {
				if splits.len() < 12 {
					return Err("invalid codec, missing profile".into());
				}

				let profile = u8::from_str_radix(splits[1], 16)
					.map_err(|e| format!("invalid codec, invalid profile: {}, {}", splits[1], e))?;

				let level = u8::from_str_radix(splits[2], 16)
					.map_err(|e| format!("invalid codec, invalid level: {}, {}", splits[2], e))?;

				let tier = match splits[3] {
					"H" => true,
					"M" => false,
					_ => return Err(format!("invalid codec, invalid tier: {}", splits[3])),
				};

				let depth = splits[4]
					.parse::<u8>()
					.map_err(|e| format!("invalid codec, invalid depth: {}, {}", splits[4], e))?;

				let monochrome = match splits[5] {
					"1" => true,
					"0" => false,
					_ => return Err(format!("invalid codec, invalid monochrome: {}", splits[5])),
				};

				let sub_sampling_x = match splits[6] {
					"1" => true,
					"0" => false,
					_ => {
						return Err(format!("invalid codec, invalid sub_sampling_x: {}", splits[6]));
					}
				};

				let sub_sampling_y = match splits[7] {
					"1" => true,
					"0" => false,
					_ => {
						return Err(format!("invalid codec, invalid sub_sampling_y: {}", splits[7]));
					}
				};

				let color_primaries = splits[8]
					.parse::<u8>()
					.map_err(|e| format!("invalid codec, invalid color_primaries: {}, {}", splits[8], e))?;

				let transfer_characteristics = splits[9]
					.parse::<u8>()
					.map_err(|e| format!("invalid codec, invalid transfer_characteristics: {}, {}", splits[9], e))?;

				let matrix_coefficients = splits[10]
					.parse::<u8>()
					.map_err(|e| format!("invalid codec, invalid matrix_coefficients: {}, {}", splits[10], e))?;

				let full_range_flag = splits[11]
					.parse::<u8>()
					.map_err(|e| format!("invalid codec, invalid full_range_flag: {}, {}", splits[11], e))?
					== 1;

				Ok(VideoCodec::Av1 {
					profile,
					level,
					tier,
					depth,
					monochrome,
					sub_sampling_x,
					sub_sampling_y,
					color_primaries,
					transfer_characteristics,
					matrix_coefficients,
					full_range_flag,
				})
			}
			r => Err(format!("invalid codec, unknown type: {}", r)),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
	Aac { object_type: AudioObjectType },
	Opus,
}

impl fmt::Display for AudioCodec {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			AudioCodec::Aac { object_type } => write!(f, "mp4a.40.{}", u16::from(*object_type)),
			AudioCodec::Opus => write!(f, "opus"),
		}
	}
}

impl FromStr for AudioCodec {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let splits = s.split('.').collect::<Vec<_>>();
		if splits.is_empty() {
			return Err("invalid codec, empty string".into());
		}

		match splits[0] {
			"mp4a" => {
				if splits.len() < 3 {
					return Err("invalid codec, missing object type".into());
				}

				let object_type = splits[2]
					.parse::<u16>()
					.map_err(|e| format!("invalid codec, invalid object type: {}, {}", splits[2], e))?;

				Ok(AudioCodec::Aac {
					object_type: AudioObjectType::from(object_type),
				})
			}
			"opus" => Ok(AudioCodec::Opus),
			r => Err(format!("invalid codec, unknown type: {}", r)),
		}
	}
}
