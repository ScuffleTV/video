use hyper::{Request, StatusCode};
use pb::scuffle::video::internal::live_rendition_manifest::RenditionInfo;
use scuffle_utils::http::ext::*;
use scuffle_utils::http::RouteError;

use super::block_style::BlockStyle;
use crate::edge::error::Result;

#[derive(Default, Debug)]
pub struct HlsConfig {
	pub block_style: Option<BlockStyle>,
	pub skip: bool,
	pub scuffle_dvr: bool,
	pub scuffle_json: bool,
}

impl HlsConfig {
	pub fn new<B>(req: &Request<B>) -> Result<Self> {
		Ok(req
			.uri()
			.query()
			.map(|v| {
				url::form_urlencoded::parse(v.as_bytes()).try_fold(HlsConfig::default(), |mut acc, (key, value)| {
					match key.as_ref() {
						"_HLS_msn" => {
							if let Some(BlockStyle::Hls { msn, .. }) = acc.block_style.as_mut() {
								*msn = value.parse().map_ignore_err_route((
									StatusCode::BAD_REQUEST,
									format!("Invalid _HLS_msn value: {}", value),
								))?;
							} else if acc.block_style.is_none() {
								acc.block_style = Some(BlockStyle::Hls {
									msn: value.parse().map_ignore_err_route((
										StatusCode::BAD_REQUEST,
										format!("Invalid _HLS_msn value: {}", value),
									))?,
									part: 0,
								});
							}
						}
						"_HLS_part" => {
							if let Some(BlockStyle::Hls { part, .. }) = acc.block_style.as_mut() {
								*part = value.parse().map_ignore_err_route((
									StatusCode::BAD_REQUEST,
									format!("Invalid _HLS_part value: {}", value),
								))?;
							} else if acc.block_style.is_none() {
								acc.block_style = Some(BlockStyle::Hls {
									msn: 0,
									part: value.parse().map_ignore_err_route((
										StatusCode::BAD_REQUEST,
										format!("Invalid _HLS_part value: {}", value),
									))?,
								});
							}
						}
						"_SCUFFLE_part" => {
							if acc.block_style.is_some() {
								return Err(RouteError::from((
									StatusCode::BAD_REQUEST,
									"Cannot use _SCUFFLE_part with _HLS_msn or _HLS_part or _SCUFFLE_ipart",
								)));
							}

							acc.block_style = Some(BlockStyle::ScufflePart(value.parse().map_ignore_err_route((
								StatusCode::BAD_REQUEST,
								format!("Invalid _SCUFFLE_part value: {}", value),
							))?));
						}
						"_SCUFFLE_ipart" => {
							if acc.block_style.is_some() {
								return Err(RouteError::from((
									StatusCode::BAD_REQUEST,
									"Cannot use _SCUFFLE_ipart with _HLS_msn or _HLS_part or _SCUFFLE_part",
								)));
							}

							acc.block_style = Some(BlockStyle::ScuffleIPart(value.parse().map_ignore_err_route((
								StatusCode::BAD_REQUEST,
								format!("Invalid _SCUFFLE_ipart value: {}", value),
							))?));
						}
						"_HLS_skip" => {
							if value == "YES" || value == "v2" {
								acc.skip = true;
							} else {
								return Err(RouteError::from((
									StatusCode::BAD_REQUEST,
									format!("Invalid _HLS_skip value: {}", value),
								)));
							}
						}
						"_SCUFFLE_dvr" => {
							if value == "YES" {
								acc.scuffle_dvr = true;
							} else {
								return Err(RouteError::from((
									StatusCode::BAD_REQUEST,
									format!("Invalid _SCUFFLE_dvr value: {}", value),
								)));
							}
						}
						"_SCUFFLE_json" => {
							if value == "YES" {
								acc.scuffle_json = true;
							} else {
								return Err(RouteError::from((
									StatusCode::BAD_REQUEST,
									format!("Invalid _SCUFFLE_json value: {}", value),
								)));
							}
						}
						_ => {}
					}

					Ok(acc)
				})
			})
			.transpose()?
			.unwrap_or_default())
	}

	pub fn is_blocked(&self, info: &RenditionInfo) -> bool {
		self.block_style
			.as_ref()
			.map(|style| style.is_blocked(info))
			.unwrap_or_default()
	}
}
