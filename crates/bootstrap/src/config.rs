pub trait ConfigParser: Sized {
	fn parse() -> impl std::future::Future<Output = anyhow::Result<Self>>;
}

impl ConfigParser for () {
	#[inline(always)]
	fn parse() -> impl std::future::Future<Output = anyhow::Result<Self>> {
		std::future::ready(Ok(()))
	}
}

pub struct EmptyConfig;

impl ConfigParser for EmptyConfig {
	#[inline(always)]
	fn parse() -> impl std::future::Future<Output = anyhow::Result<Self>> {
		std::future::ready(Ok(EmptyConfig))
	}
}
