use mp4::codec::VideoCodec;
use pb::scuffle::video::v1::types::{AudioConfig, Rendition, TranscodingConfig, VideoConfig};

pub fn determine_output_renditions(
	video_input: &VideoConfig,
	audio_input: &AudioConfig,
	transcoding_config: &TranscodingConfig,
) -> (Vec<VideoConfig>, Vec<AudioConfig>) {
	let mut audio_configs = vec![];
	let mut video_configs = vec![];

	if transcoding_config.renditions.contains(&Rendition::AudioSource.into()) {
		audio_configs.push(AudioConfig {
			rendition: Rendition::AudioSource as i32,
			codec: audio_input.codec.clone(),
			bitrate: audio_input.bitrate,
			channels: audio_input.channels,
			sample_rate: audio_input.sample_rate,
		});
	}

	if transcoding_config.renditions.contains(&Rendition::VideoSource.into()) {
		video_configs.push(VideoConfig {
			rendition: Rendition::VideoSource as i32,
			codec: video_input.codec.clone(),
			bitrate: video_input.bitrate,
			fps: video_input.fps,
			height: video_input.height,
			width: video_input.width,
		});
	}

	let aspect_ratio = video_input.width as f64 / video_input.height as f64;

	struct Resolution {
		rendition: Rendition,
		side: u32,
		framerate: u32,
		bitrate: u32,
	}

	let mut resolutions = vec![];

	if transcoding_config.renditions.contains(&Rendition::VideoHd.into()) {
		resolutions.push(Resolution {
			rendition: Rendition::VideoHd,
			bitrate: 4000 * 1024,
			framerate: video_input.fps.min(60) as u32,
			side: 720,
		});
	}

	if transcoding_config.renditions.contains(&Rendition::VideoSd.into()) {
		resolutions.push(Resolution {
			rendition: Rendition::VideoSd,
			bitrate: 2000 * 1024,
			framerate: video_input.fps.min(30) as u32,
			side: 480,
		});
	}

	if transcoding_config.renditions.contains(&Rendition::VideoLd.into()) {
		resolutions.push(Resolution {
			rendition: Rendition::VideoLd,
			bitrate: 1000 * 1024,
			framerate: video_input.fps.min(30) as u32,
			side: 360,
		})
	}

	for res in resolutions {
		// This prevents us from upscaling the video
		// We only want to downscale the video
		let (mut width, mut height) = if aspect_ratio > 1.0 && video_input.height as u32 > res.side {
			((res.side as f64 * aspect_ratio).round() as u32, res.side)
		} else if aspect_ratio < 1.0 && video_input.width as u32 > res.side {
			(res.side, (res.side as f64 / aspect_ratio).round() as u32)
		} else {
			continue;
		};

		// we need even numbers for the width and height
		// this is a requirement of the h264 codec
		if width % 2 != 0 {
			width += 1;
		} else if height % 2 != 0 {
			height += 1;
		}

		// We dont want to transcode video with resolutions less than 100px on either
		// side We also do not want to transcode anything more expensive than 720p on a
		// 16:9 aspect ratio (720 * 1280) This prevents us from transcoding a "720p"
		// with an aspect ratio of 4:1 (720 * 2880) which is extremely expensive.
		// Just some insight, 2880 / 1280 = 2.25, so this video is 2.25 times more
		// expensive than a normal 720p video. 1080 * 1920 = 2073600
		// 720 * 2880 = 2073600
		// So a 720p video with an aspect ratio of 4:1 is just as expensive as a 1080p
		// video with a 16:9 aspect ratio.
		if width < 100 || height < 100 || width * height > 720 * 1280 {
			continue;
		}

		video_configs.push(VideoConfig {
			rendition: res.rendition as i32,
			codec: VideoCodec::Avc {
				profile: 100, // High
				level: 51,    // 5.1
				constraint_set: 0,
			}
			.to_string(),
			bitrate: res.bitrate as i64,
			fps: res.framerate as i32,
			height: height as i32,
			width: width as i32,
		})
	}

	video_configs.sort_by(|a, b| b.width.cmp(&a.width));

	(video_configs, audio_configs)
}
