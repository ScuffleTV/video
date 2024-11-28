use tonic::Status;

pub fn validate_name(name: &str) -> tonic::Result<()> {
	if name.is_empty() {
		return Err(Status::invalid_argument("name cannot be empty"));
	}

	if name.len() > 255 {
		return Err(Status::invalid_argument("name cannot be longer than 255 characters"));
	}

	name.chars()
		.find(|c| !c.is_ascii_alphanumeric() && *c != '-' && *c != '_')
		.map_or(Ok(()), |_| {
			Err(Status::invalid_argument("name must only contain ASCII characters"))
		})
}

pub fn validate_access_key_id(access_key_id: &str) -> tonic::Result<()> {
	if access_key_id.is_empty() {
		return Err(Status::invalid_argument("access key id cannot be empty"));
	}

	if access_key_id.len() > 255 {
		return Err(Status::invalid_argument("access key id cannot be longer than 255 characters"));
	}

	Ok(())
}

pub fn validate_secret_access_key(secret_access_key: &str) -> tonic::Result<()> {
	if secret_access_key.is_empty() {
		return Err(Status::invalid_argument("secret access key cannot be empty"));
	}

	if secret_access_key.len() > 255 {
		return Err(Status::invalid_argument(
			"secret access key cannot be longer than 255 characters",
		));
	}

	Ok(())
}

pub fn validate_region(region: &str) -> tonic::Result<()> {
	if region.is_empty() {
		return Err(Status::invalid_argument("region cannot be empty"));
	}

	Ok(())
}

pub fn validate_endpoint(endpoint: &str) -> tonic::Result<()> {
	url::Url::parse(endpoint)
		.map_err(|_| Status::invalid_argument("invalid endpoint"))
		.map(|_| ())
}

pub fn validate_public_url(public_url: &str) -> tonic::Result<()> {
	url::Url::parse(public_url)
		.map_err(|_| Status::invalid_argument("invalid public url"))
		.map(|_| ())
}
