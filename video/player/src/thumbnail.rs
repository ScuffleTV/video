use url::Url;
use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Debug, Clone, serde::Deserialize, tsify::Tsify)]
#[tsify(from_wasm_abi)]
pub struct RoomThubnailSettings {
	pub server: Option<String>,
	pub organization_id: String,
	pub room_id: String,
	pub token: Option<String>,
}

#[wasm_bindgen]
pub fn get_room_thumbnail(settings: RoomThubnailSettings) -> String {
	let url = Url::parse(settings.server.as_deref().unwrap_or("https://edge.scuffle.video")).unwrap();

	let mut url = url
		.join(&format!("/{}/{}.jpg", settings.organization_id, settings.room_id))
		.unwrap();

	if let Some(token) = &settings.token {
		url.query_pairs_mut().append_pair("token", token);
	}

	url.to_string()
}
