use hex::ToHex;
use sha2::Digest;
use tonic::Status;

pub fn validate_public_key(public_key: &str) -> tonic::Result<(String, String)> {
	let public_key =
		jwt_next::asymmetric::PublicKey::from_pem(public_key).map_err(|_| Status::invalid_argument("invalid public key"))?;

	match public_key {
		jwt_next::asymmetric::PublicKey::RSA(_) => Err(Status::invalid_argument("RSA keys are not supported, use EC384")),
		jwt_next::asymmetric::PublicKey::EC256(_) => {
			Err(Status::invalid_argument("EC256 keys are not supported, use EC384"))
		}
		jwt_next::asymmetric::PublicKey::EC384(key) => Ok((
			key.to_string(),
			sha2::Sha256::digest(key.to_sec1_bytes()).to_vec().encode_hex(),
		)),
	}
}
