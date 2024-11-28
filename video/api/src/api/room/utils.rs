use rand::Rng;

pub fn create_stream_key() -> String {
	rand::thread_rng()
		.sample_iter(&rand::distributions::Alphanumeric)
		.take(32)
		.map(char::from)
		.collect::<String>()
}
