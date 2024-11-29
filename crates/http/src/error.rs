use std::convert::Infallible;

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
	#[error(transparent)]
	Http(#[from] http::Error),
	#[cfg(feature = "http3")]
	#[error(transparent)]
	H3(#[from] h3::Error),
	#[cfg(any(feature = "http1", feature = "http2"))]
	#[error(transparent)]
	Hyper(#[from] hyper::Error),
	#[error("closed")]
	Closed,
	#[error(transparent)]
	Unknown(#[from] Box<dyn std::error::Error + Send + Sync>),
	#[cfg(feature = "axum")]
	#[error(transparent)]
	Axum(#[from] axum_core::Error),
	#[cfg(feature = "quic-quinn")]
	#[error(transparent)]
	QuinnConnection(#[from] quinn::ConnectionError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error("timeout")]
	Timeout,
	#[error("configuration")]
	Configuration,
}

impl From<tokio::time::error::Elapsed> for ErrorKind {
	fn from(_: tokio::time::error::Elapsed) -> Self {
		Self::Timeout
	}
}

impl From<Infallible> for ErrorKind {
	fn from(_: Infallible) -> Self {
		unreachable!()
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
