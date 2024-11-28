use std::sync::Arc;

use chrono::{Duration, TimeZone, Utc};
use hmac::{Hmac, Mac};
use hyper::StatusCode;
use jwt_next::asymmetric::VerifyingKey;
use jwt_next::{asymmetric, AlgorithmType, SignWithKey, Token, VerifyWithKey};
use scuffle_utils::http::ext::*;
use sha2::Sha256;
use ulid::Ulid;
use video_common::database::{PlaybackKeyPair, Rendition};

use crate::config::EdgeConfig;
use crate::edge::error::Result;
use crate::global::EdgeGlobal;

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct TokenClaims {
	/// The room name that this token is for (required)
	pub organization_id: Option<Ulid>,

	/// The room id that this token is for (required, either this or the
	/// recording id must be present)
	pub room_id: Option<Ulid>,

	/// The ingest connection id that this token is for (required, either this
	/// or the room id must be present)
	pub recording_id: Option<Ulid>,

	/// The time at which the token was issued (required)
	pub iat: Option<i64>,

	/// Used to create single use tokens (optional)
	pub id: Option<String>,

	/// The user ID that this token is for (optional)
	pub user_id: Option<String>,
}

pub enum TargetId {
	Room(Ulid),
	Recording(Ulid),
}

impl TokenClaims {
	pub async fn verify(
		client: &utils::database::tokio_postgres::Client,
		organization_id: Ulid,
		target_id: TargetId,
		token: &str,
	) -> Result<Token<jwt_next::Header, Self, jwt_next::Verified>> {
		let token: Token<jwt_next::Header, Self, _> = Token::parse_unverified(token).map_err(|e| {
			tracing::error!(error = %e, "failed to parse token");
			(StatusCode::BAD_REQUEST, "invalid token, could not parse")
		})?;

		let playback_key_pair_id = Ulid::from_string(
			token
				.header()
				.key_id
				.as_ref()
				.ok_or((StatusCode::BAD_REQUEST, "invalid token, missing key id"))?,
		)
		.map_err(|_| (StatusCode::BAD_REQUEST, "invalid token, invalid key id"))?;

		if token.header().algorithm != AlgorithmType::Es384 {
			return Err((
				StatusCode::BAD_REQUEST,
				"invalid token, invalid algorithm, only ES384 is supported",
			)
				.into());
		}

		if organization_id
			!= token
				.claims()
				.organization_id
				.ok_or((StatusCode::BAD_REQUEST, "invalid token, missing organization id"))?
		{
			return Err((StatusCode::BAD_REQUEST, "invalid token, organization id mismatch").into());
		}

		if token.claims().room_id.is_some() && token.claims().recording_id.is_some() {
			return Err((
				StatusCode::BAD_REQUEST,
				"invalid token, both room id and recording id are present",
			)
				.into());
		}

		match target_id {
			TargetId::Room(id) => {
				if id
					!= token
						.claims()
						.room_id
						.ok_or((StatusCode::BAD_REQUEST, "invalid token, missing room id"))?
				{
					return Err((StatusCode::BAD_REQUEST, "invalid token, room id mismatch").into());
				}
			}
			TargetId::Recording(id) => {
				if id
					!= token
						.claims()
						.recording_id
						.ok_or((StatusCode::BAD_REQUEST, "invalid token, missing recording id"))?
				{
					return Err((StatusCode::BAD_REQUEST, "invalid token, recording id mismatch").into());
				}
			}
		}

		let iat = Utc
			.timestamp_opt(
				token
					.claims()
					.iat
					.ok_or((StatusCode::BAD_REQUEST, "invalid token, missing iat"))?,
				0,
			)
			.single()
			.ok_or((StatusCode::BAD_REQUEST, "invalid token, iat is invalid"))?;

		if iat > chrono::Utc::now() {
			return Err((StatusCode::BAD_REQUEST, "invalid token, iat is in the future").into());
		}

		if iat < chrono::Utc::now() - Duration::minutes(5) {
			return Err((StatusCode::BAD_REQUEST, "invalid token, iat is too far in the past").into());
		}

		let keypair: Option<PlaybackKeyPair> = scuffle_utils::database::query(
			r#"
			SELECT
				*
			FROM
				playback_key_pairs
			WHERE
				organization_id = $1
				AND id = $2
			"#,
		)
		.bind(organization_id)
		.bind(playback_key_pair_id)
		.build_query_as()
		.fetch_optional(client)
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?;

		let keypair = keypair.ok_or((StatusCode::BAD_REQUEST, "invalid token, keypair not found"))?;

		let public_key = asymmetric::PublicKey::from_pem_bytes(&keypair.public_key)
			.map_ignore_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to parse public key"))?
			.into_ec384()
			.map_ignore_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to parse public key"))?;

		let verifier = asymmetric::AsymmetricKeyWithDigest::new(VerifyingKey::from_ec384(public_key));

		let token = token
			.verify_with_key(&verifier)
			.map_err(|_| (StatusCode::BAD_REQUEST, "invalid token, failed to verify"))?;

		let mut qb = scuffle_utils::database::QueryBuilder::default();

		qb.push("SELECT 1 FROM playback_session_revocations WHERE organization_id = ")
			.push_bind(organization_id)
			.push(" AND revoke_before < ")
			.push_bind(iat);

		if let Some(uid) = token.claims().user_id.as_ref() {
			qb.push(" AND user_id = ").push_bind(uid);
		} else {
			qb.push(" AND user_id IS NULL");
		}

		match target_id {
			TargetId::Recording(recording_id) => {
				qb.push(" AND (recording_id = ")
					.push_bind(recording_id)
					.push(" OR recording_id IS NULL) AND room_id IS NULL");
			}
			TargetId::Room(room_id) => {
				qb.push(" AND (room_id = ")
					.push_bind(room_id)
					.push(" OR room_id IS NULL) AND recording_id IS NULL");
			}
		}

		qb.push(" AND sso_id IS NULL LIMIT 1");

		if qb
			.build()
			.fetch_optional(client)
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?
			.is_some()
		{
			return Err((StatusCode::BAD_REQUEST, "invalid token, token has been revoked").into());
		}

		if let Some(id) = token.claims().id.as_ref() {
			if scuffle_utils::database::query(
				"INSERT INTO playback_session_revocations(organization_id, sso_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
			)
			.bind(organization_id)
			.bind(id)
			.build()
			.execute(client)
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?
				!= 1
			{
				return Err((StatusCode::BAD_REQUEST, "token has already been used").into());
			}
		}

		drop(qb);

		Ok(token)
	}
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MediaClaims {
	/// The organization id of the stream.
	#[serde(rename = "o")]
	pub organization_id: Ulid,

	/// The room id of the stream.
	#[serde(rename = "r")]
	pub room_id: Ulid,

	/// The ingest connection of the stream.
	#[serde(rename = "c")]
	pub connection_id: Ulid,

	/// The rendition that is allowed to be accessed
	#[serde(rename = "d")]
	pub rendition: Rendition,

	/// The type of the media being this token is for
	#[serde(rename = "t")]
	pub ty: MediaClaimsType,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum MediaClaimsType {
	Init,
	Part(u32),
	Segment(u32),
}

impl MediaClaims {
	pub fn verify<G: EdgeGlobal>(global: &Arc<G>, organization_id: Ulid, room_id: Ulid, token: &str) -> Result<Self> {
		let key: Hmac<Sha256> = Hmac::new_from_slice(global.config::<EdgeConfig>().media_key.as_bytes())
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to create hmac"))?;

		let token: Token<jwt_next::Header, Self, _> = token
			.verify_with_key(&key)
			.map_err(|_| (StatusCode::BAD_REQUEST, "invalid token, could not parse"))?;

		if organization_id != token.claims().organization_id {
			return Err((StatusCode::BAD_REQUEST, "invalid token, organization id mismatch").into());
		}

		if room_id != token.claims().room_id {
			return Err((StatusCode::BAD_REQUEST, "invalid token, room id mismatch").into());
		}

		Ok(token.claims().clone())
	}

	pub fn sign<G: EdgeGlobal>(&self, global: &Arc<G>) -> Result<String> {
		let key: Hmac<Sha256> = Hmac::new_from_slice(global.config::<EdgeConfig>().media_key.as_bytes())
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to create hmac"))?;

		let token = self
			.sign_with_key(&key)
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to sign token"))?;

		Ok(token)
	}
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ScreenshotClaims {
	/// The organization id of the stream.
	#[serde(rename = "o")]
	pub organization_id: Ulid,

	/// The room id of the stream.
	#[serde(rename = "r")]
	pub room_id: Ulid,

	/// The ingest connection of the stream.
	#[serde(rename = "c")]
	pub connection_id: Ulid,

	/// The index of the screenshot that is allowed to be accessed
	#[serde(rename = "i")]
	pub idx: u32,
}

impl ScreenshotClaims {
	pub fn verify<G: EdgeGlobal>(global: &Arc<G>, organization_id: Ulid, room_id: Ulid, token: &str) -> Result<Self> {
		let key: Hmac<Sha256> = Hmac::new_from_slice(global.config::<EdgeConfig>().media_key.as_bytes())
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to create hmac"))?;

		let token: Token<jwt_next::Header, Self, _> = token
			.verify_with_key(&key)
			.map_err(|_| (StatusCode::BAD_REQUEST, "invalid token, could not parse"))?;

		if organization_id != token.claims().organization_id {
			return Err((StatusCode::BAD_REQUEST, "invalid token, organization id mismatch").into());
		}

		if room_id != token.claims().room_id {
			return Err((StatusCode::BAD_REQUEST, "invalid token, room id mismatch").into());
		}

		Ok(token.claims().clone())
	}

	pub fn sign<G: EdgeGlobal>(&self, global: &Arc<G>) -> Result<String> {
		let key: Hmac<Sha256> = Hmac::new_from_slice(global.config::<EdgeConfig>().media_key.as_bytes())
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to create hmac"))?;

		let token = self
			.sign_with_key(&key)
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to sign token"))?;

		Ok(token)
	}
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SessionClaims {
	/// The id of the session
	#[serde(rename = "i")]
	pub id: Ulid,

	/// The organization id of the stream.
	#[serde(rename = "o")]
	pub organization_id: Ulid,

	/// The type of the session
	#[serde(flatten)]
	pub ty: SessionClaimsType,

	/// The time at which the token was issued
	#[serde(rename = "a")]
	pub iat: i64,

	/// If the user was authenticated when the session was created
	#[serde(rename = "u")]
	pub was_authenticated: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, Copy, PartialEq, Eq)]
#[serde(untagged)]
pub enum SessionClaimsType {
	Room {
		#[serde(rename = "r")]
		room_id: Ulid,
		#[serde(rename = "c")]
		connection_id: Ulid,
	},
	Recording {
		#[serde(rename = "r")]
		recording_id: Ulid,
	},
}

impl SessionClaims {
	pub fn verify<G: EdgeGlobal>(global: &Arc<G>, organization_id: Ulid, token: &str) -> Result<Self> {
		let key: Hmac<Sha256> = Hmac::new_from_slice(global.config::<EdgeConfig>().session_key.as_bytes())
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to create hmac"))?;

		let token: Token<jwt_next::Header, Self, _> = token
			.verify_with_key(&key)
			.map_err(|_| (StatusCode::BAD_REQUEST, "invalid token, could not parse"))?;

		if organization_id != token.claims().organization_id {
			return Err((StatusCode::BAD_REQUEST, "invalid token, organization id mismatch").into());
		}

		Ok(token.claims().clone())
	}

	pub fn sign<G: EdgeGlobal>(&self, global: &Arc<G>) -> Result<String> {
		let key: Hmac<Sha256> = Hmac::new_from_slice(global.config::<EdgeConfig>().session_key.as_bytes())
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to create hmac"))?;

		let token = self
			.sign_with_key(&key)
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to sign token"))?;

		Ok(token)
	}
}
