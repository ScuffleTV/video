use std::convert::Infallible;
use std::error::Error as StdError;

#[derive(Debug)]
pub struct Error {
	inner: Box<ErrorInner>,
}

#[derive(Debug)]
struct ErrorInner {
	kind: Option<ErrorKind>,
	context: smallvec::SmallVec<[&'static str; 8]>,
	#[cfg(feature = "error-backtrace")]
	backtrace: std::backtrace::Backtrace,
	callsite: &'static std::panic::Location<'static>,
	severity: ErrorSeverity,
	scope: ErrorScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ErrorSeverity {
	#[default]
	Unknown,
	Error,
	Warning,
	Info,
	Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ErrorScope {
	Protocol,
	Connection,
	Request,
	Response,
	#[default]
	Unknown,
}

impl std::fmt::Display for ErrorScope {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Protocol => write!(f, "protocol"),
			Self::Connection => write!(f, "connection"),
			Self::Request => write!(f, "request"),
			Self::Response => write!(f, "response"),
			Self::Unknown => write!(f, "unknown"),
		}
	}
}

pub struct ErrorConfig {
	pub severity: ErrorSeverity,
	pub scope: ErrorScope,
	pub context: &'static str,
}

impl ErrorConfig {
	pub fn build(self) -> Error {
		Error::new().with_config(self)
	}
}

impl From<ErrorConfig> for Error {
	fn from(config: ErrorConfig) -> Self {
		config.build()
	}
}

impl Error {
	#[track_caller]
	pub(crate) fn new() -> Self {
		Self {
			inner: Box::new(ErrorInner {
				kind: None,
				context: smallvec::SmallVec::new(),
				#[cfg(feature = "error-backtrace")]
				backtrace: std::backtrace::Backtrace::capture(),
				callsite: std::panic::Location::caller(),
				severity: ErrorSeverity::Error,
				scope: ErrorScope::Unknown,
			}),
		}
	}

	pub fn with_kind(kind: ErrorKind) -> Self {
		let mut error = Self::new();
		error.inner.severity = kind.severity();
		error.inner.kind = Some(kind);
		error
	}

	pub fn with_severity(mut self, severity: ErrorSeverity) -> Self {
		self.inner.severity = severity;
		self
	}

	pub fn with_scope(mut self, scope: ErrorScope) -> Self {
		self.inner.scope = scope;
		self
	}

	pub fn with_context(mut self, request: &'static str) -> Self {
		self.inner.context.push(request);
		self
	}

	pub fn into_kind(self) -> Option<ErrorKind> {
		self.inner.kind
	}

	pub fn kind(&self) -> Option<&ErrorKind> {
		self.inner.kind.as_ref()
	}

	pub fn context(&self) -> &[&'static str] {
		&self.inner.context
	}

	pub fn severity(&self) -> ErrorSeverity {
		self.inner.severity
	}

	#[cfg(feature = "error-backtrace")]
	pub fn backtrace(&self) -> &std::backtrace::Backtrace {
		&self.inner.backtrace
	}

	pub fn callsite(&self) -> &'static std::panic::Location<'static> {
		self.inner.callsite
	}

	pub fn with_config(self, config: ErrorConfig) -> Self {
		self.with_severity(config.severity)
			.with_scope(config.scope)
			.with_context(config.context)
	}
}

#[allow(dead_code)]
pub(crate) trait ResultErrorExt<R>: Sized {
	fn downcast(self) -> Result<R, Error>;

	#[track_caller]
	fn with_scope(self, scope: ErrorScope) -> Result<R, Error> {
		self.downcast().map_err(|e| e.with_scope(scope))
	}

	#[track_caller]
	fn with_context(self, request: &'static str) -> Result<R, Error> {
		self.downcast().map_err(|e| e.with_context(request))
	}

	#[track_caller]
	fn with_severity(self, severity: ErrorSeverity) -> Result<R, Error> {
		self.downcast().map_err(|e| e.with_severity(severity))
	}

	#[track_caller]
	fn with_config(self, config: ErrorConfig) -> Result<R, Error> {
		self.downcast().map_err(|e| e.with_config(config))
	}
}

impl<R, E: std::error::Error + Send + Sync + 'static> ResultErrorExt<R> for Result<R, E> {
	fn downcast(self) -> Result<R, Error> {
		self.map_err(|error| downcast(Box::new(error)))
	}
}

pub(crate) fn downcast(error: Box<dyn std::error::Error + Send + Sync + 'static>) -> Error {
	if error.is::<Error>() {
		return *error.downcast::<Error>().unwrap();
	}

	if error.is::<ErrorKind>() {
		return Error::with_kind(*error.downcast().unwrap());
	}

	if error.is::<http::Error>() {
		return Error::with_kind(ErrorKind::Http(*error.downcast().unwrap()));
	}

	#[cfg(feature = "http3")]
	if error.is::<h3::Error>() {
		return Error::with_kind(ErrorKind::H3(*error.downcast().unwrap()));
	}

	#[cfg(any(feature = "http1", feature = "http2"))]
	if error.is::<hyper::Error>() {
		return Error::with_kind(ErrorKind::Hyper(*error.downcast().unwrap()));
	}

	#[cfg(feature = "quic-quinn")]
	if error.is::<quinn::ConnectionError>() {
		return Error::with_kind(ErrorKind::QuinnConnection(*error.downcast().unwrap()));
	}

	if error.is::<std::io::Error>() {
		return Error::with_kind(ErrorKind::Io(*error.downcast().unwrap()));
	}

	#[cfg(feature = "axum")]
	if error.is::<axum_core::Error>() {
		return Error::with_kind(ErrorKind::Axum(*error.downcast().unwrap()));
	}

	if error.is::<tokio::time::error::Elapsed>() {
		return Error::with_kind(ErrorKind::Timeout);
	}

	Error::with_kind(ErrorKind::Unknown(error))
}

#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) fn find_source(mut error: &(dyn std::error::Error + 'static)) -> Option<ErrorSeverity> {
	loop {
		if let Some(err) = error.downcast_ref::<Error>() {
			return Some(err.severity());
		}

		if let Some(err) = error.downcast_ref::<http::Error>() {
			return Some(err.severity());
		}

		#[cfg(feature = "http3")]
		if let Some(err) = error.downcast_ref::<h3::Error>() {
			return Some(err.severity());
		}

		#[cfg(any(feature = "http1", feature = "http2"))]
		if let Some(err) = error.downcast_ref::<hyper::Error>() {
			return Some(err.severity());
		}

		#[cfg(feature = "quic-quinn")]
		if let Some(err) = error.downcast_ref::<quinn::ConnectionError>() {
			return Some(err.severity());
		}

		if let Some(err) = error.downcast_ref::<std::io::Error>() {
			return Some(err.severity());
		}

		#[cfg(feature = "axum")]
		if let Some(err) = error.downcast_ref::<axum_core::Error>() {
			return Some(err.severity());
		}

		if error.is::<tokio::time::error::Elapsed>() {
			return Some(ErrorSeverity::Debug);
		}

		let Some(err) = error.source() else {
			break;
		};

		error = err;
	}

	None
}

impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		self.inner.kind.as_ref().map(|k| k as &(dyn std::error::Error + 'static))
	}
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut first = true;

		if self.inner.scope != ErrorScope::Unknown {
			first = false;
			write!(f, "{}", self.inner.scope)?;
		}

		for context in self.inner.context.iter().rev() {
			if !first {
				write!(f, ": ")?;
			}

			first = false;
			write!(f, "{}", context)?;
		}

		if let Some(kind) = self.inner.kind.as_ref() {
			if !first {
				write!(f, ": ")?;
			}

			write!(f, "{}", kind)?;
		}

		Ok(())
	}
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
	#[error("http: {0}")]
	Http(#[from] http::Error),
	#[cfg(feature = "http3")]
	#[error("h3: {0}")]
	H3(#[from] h3::Error),
	#[cfg(any(feature = "http1", feature = "http2"))]
	#[error("hyper: {0}")]
	Hyper(#[from] hyper::Error),
	#[error("closed")]
	Closed,
	#[error(transparent)]
	Unknown(#[from] Box<dyn std::error::Error + Send + Sync>),
	#[cfg(feature = "axum")]
	#[error("axum: {0}")]
	Axum(#[from] axum_core::Error),
	#[cfg(feature = "quic-quinn")]
	#[error("quinn: {0}")]
	QuinnConnection(#[from] quinn::ConnectionError),
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
	#[error("timeout")]
	Timeout,
	#[error("configuration")]
	Configuration,
	#[error("bad request")]
	BadRequest,
}

trait ErrorKindExt {
	fn severity(&self) -> ErrorSeverity;
}

impl ErrorKindExt for http::Error {
	fn severity(&self) -> ErrorSeverity {
		ErrorSeverity::Debug
	}
}

#[cfg(feature = "http3")]
impl ErrorKindExt for h3::Error {
	fn severity(&self) -> ErrorSeverity {
		match self.kind() {
			h3::error::Kind::Application { code, .. } => match code {
				h3::error::Code::H3_NO_ERROR => ErrorSeverity::Debug,
				h3::error::Code::H3_REQUEST_CANCELLED => ErrorSeverity::Debug,
				h3::error::Code::H3_REQUEST_INCOMPLETE => ErrorSeverity::Debug,
				_ => ErrorSeverity::Error,
			},
			_ => {
				if let Some(severity) = self.source().and_then(find_source) {
					severity
				} else {
					ErrorSeverity::Error
				}
			}
		}
	}
}

#[cfg(any(feature = "http1", feature = "http2"))]
impl ErrorKindExt for hyper::Error {
	fn severity(&self) -> ErrorSeverity {
		if self.is_incomplete_message() {
			ErrorSeverity::Debug
		} else {
			self.source().and_then(find_source).unwrap_or(ErrorSeverity::Error)
		}
	}
}

#[cfg(feature = "quic-quinn")]
impl ErrorKindExt for quinn::ConnectionError {
	fn severity(&self) -> ErrorSeverity {
		match self {
			Self::CidsExhausted => ErrorSeverity::Error,
			Self::TimedOut => ErrorSeverity::Debug,
			Self::ConnectionClosed(..) => ErrorSeverity::Debug,
			Self::Reset => ErrorSeverity::Debug,
			Self::VersionMismatch => ErrorSeverity::Debug,
			Self::TransportError(transport) => transport.code.severity(),
			Self::ApplicationClosed(..) => ErrorSeverity::Debug,
			Self::LocallyClosed => ErrorSeverity::Debug,
		}
	}
}

#[cfg(feature = "quic-quinn")]
impl ErrorKindExt for quinn::TransportErrorCode {
	fn severity(&self) -> ErrorSeverity {
		match self {
			&Self::NO_ERROR => ErrorSeverity::Debug,
			&Self::FLOW_CONTROL_ERROR => ErrorSeverity::Debug,
			&Self::STREAM_LIMIT_ERROR => ErrorSeverity::Debug,
			&Self::STREAM_STATE_ERROR => ErrorSeverity::Debug,
			_ => ErrorSeverity::Error,
		}
	}
}

impl ErrorKindExt for std::io::Error {
	fn severity(&self) -> ErrorSeverity {
		match self.kind() {
			std::io::ErrorKind::TimedOut => ErrorSeverity::Debug,
			std::io::ErrorKind::ConnectionReset => ErrorSeverity::Debug,
			std::io::ErrorKind::ConnectionAborted => ErrorSeverity::Debug,
			std::io::ErrorKind::UnexpectedEof => ErrorSeverity::Debug,
			std::io::ErrorKind::BrokenPipe => ErrorSeverity::Debug,
			_ => ErrorSeverity::Error,
		}
	}
}

#[cfg(feature = "axum")]
impl ErrorKindExt for axum_core::Error {
	fn severity(&self) -> ErrorSeverity {
		ErrorSeverity::Error
	}
}

impl ErrorKind {
	pub fn severity(&self) -> ErrorSeverity {
		match self {
			Self::Timeout => ErrorSeverity::Debug,
			Self::BadRequest => ErrorSeverity::Debug,
			Self::Configuration => ErrorSeverity::Error,
			Self::Closed => ErrorSeverity::Debug,
			Self::Unknown(_) => ErrorSeverity::Error,
			#[cfg(feature = "http3")]
			Self::H3(err) => err.severity(),
			#[cfg(any(feature = "http1", feature = "http2"))]
			Self::Hyper(err) => err.severity(),
			#[cfg(feature = "axum")]
			Self::Axum(err) => err.severity(),
			#[cfg(feature = "quic-quinn")]
			Self::QuinnConnection(err) => err.severity(),
			Self::Io(io) => io.severity(),
			Self::Http(err) => err.severity(),
		}
	}
}

impl From<tokio::time::error::Elapsed> for ErrorKind {
	fn from(_: tokio::time::error::Elapsed) -> Self {
		Self::Timeout
	}
}

impl From<Infallible> for ErrorKind {
	fn from(_: Infallible) -> Self {
		unreachable!("infallible cannot be constructed")
	}
}

impl<E: Into<ErrorKind>> From<E> for Error {
	fn from(inner: E) -> Self {
		Self::with_kind(inner.into())
	}
}

impl From<&'static str> for Error {
	fn from(inner: &'static str) -> Self {
		Self::new().with_context(inner)
	}
}
