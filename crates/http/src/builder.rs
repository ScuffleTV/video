#[derive(derive_more::Debug)]
pub enum MakeListener<T> {
	Bind(std::net::SocketAddr),
	#[debug("Custom")]
	Custom(Box<dyn FnOnce() -> std::io::Result<T> + 'static + Send>),
	#[debug("Listener")]
	Listener(T),
}

impl<T> MakeListener<T> {
	pub fn bind(addr: std::net::SocketAddr) -> MakeListener<T> {
		MakeListener::Bind(addr)
	}

	pub fn custom(make_listener: impl Fn() -> std::io::Result<T> + 'static + Send) -> MakeListener<T> {
		MakeListener::Custom(Box::new(make_listener))
	}

	pub fn listener(listener: T) -> MakeListener<T> {
		MakeListener::Listener(listener)
	}
}

impl MakeListener<std::net::UdpSocket> {
	pub fn make(&mut self) -> std::io::Result<std::net::UdpSocket> {
		match self {
			MakeListener::Bind(addr) => {
				let listener = std::net::UdpSocket::bind(*addr)?;
				*self = MakeListener::Listener(listener.try_clone()?);
			}
			MakeListener::Custom(make_listener) => {
				let make_listener = std::mem::replace(make_listener, Box::new(|| panic!("called after use")));
				*self = MakeListener::Listener(make_listener()?);
			}
			MakeListener::Listener(_) => {}
		}

		if let MakeListener::Listener(listener) = self {
			listener.try_clone()
		} else {
			unreachable!("Invalid MakeListener state, please open an issue on GitHub")
		}
	}
}

impl MakeListener<std::net::TcpListener> {
	pub fn make(&mut self) -> std::io::Result<std::net::TcpListener> {
		match self {
			MakeListener::Bind(addr) => {
				let listener = std::net::TcpListener::bind(*addr)?;
				*self = MakeListener::Listener(listener.try_clone()?);
			}
			MakeListener::Custom(make_listener) => {
				let make_listener = std::mem::replace(make_listener, Box::new(|| panic!("called after use")));
				*self = MakeListener::Listener(make_listener()?);
			}
			MakeListener::Listener(_) => {}
		}

		if let MakeListener::Listener(listener) = self {
			listener.try_clone()
		} else {
			unreachable!("Invalid MakeListener state, please open an issue on GitHub")
		}
	}
}
