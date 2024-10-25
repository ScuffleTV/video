use std::borrow::Cow;

pub use opentelemetry::trace::{Link, SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState};
pub use opentelemetry::KeyValue;
pub use opentelemetry_sdk::export::trace::SpanData;
pub use opentelemetry_sdk::trace::SpanEvents;
use rand::Rng;
use spin::Lazy;
use tracing::{span, Metadata};

#[derive(Debug, Clone)]
pub struct SpanNode {
	pub id: span::Id,
	pub status: Status,
	pub name: Cow<'static, str>,
	pub trace_id: TraceId,
	pub mapped_id: SpanId,
	pub metadata: &'static Metadata<'static>,
	pub attributes: Vec<opentelemetry::KeyValue>,
	pub end: Option<std::time::SystemTime>,
	pub last_event_time: std::time::Instant,
	pub active_time: std::time::Duration,
	pub idle_time: std::time::Duration,
	pub events: Vec<SpanEvent>,
	pub links: Vec<SpanContext>,
	pub start: Option<std::time::SystemTime>,
	pub mapped_parent_id: Option<SpanId>,
	pub root: Option<RootNode>,
}

#[derive(Debug, Clone)]
pub enum RootNode {
	Root(Vec<SpanNode>),
	Child(span::Id),
}

#[derive(Debug, Clone)]
pub struct SpanEvent {
	pub time: std::time::SystemTime,
	pub name: Cow<'static, str>,
	pub metadata: &'static Metadata<'static>,
	pub attributes: Vec<opentelemetry::KeyValue>,
}

impl SpanEvent {
	fn into_data(mut self) -> opentelemetry::trace::Event {
		if let Some(file) = self.metadata.file() {
			self.attributes.push(KeyValue::new("code.filepath", file))
		}
		if let Some(line) = self.metadata.line() {
			self.attributes.push(KeyValue::new("code.lineno", line as i64))
		}
		if let Some(module) = self.metadata.module_path() {
			self.attributes.push(KeyValue::new("code.namespace", module))
		}
		self.attributes.push(KeyValue::new("level", self.metadata.level().as_str()));

		opentelemetry::trace::Event::new(self.name, self.time, self.attributes, 0)
	}
}

fn gen_trace_id() -> TraceId {
	let mut bytes = [0u8; 16];
	rand::thread_rng().fill(&mut bytes);
	TraceId::from_bytes(bytes)
}

impl SpanNode {
	pub fn new(
		id: span::Id,
		trace_id: Option<TraceId>,
		mapped_id: SpanId,
		mapped_parent_id: Option<SpanId>,
		attrs: &span::Attributes<'_>,
		root_child: Option<span::Id>,
	) -> Self {
		let mut this = Self {
			id,
			status: Status::Unset,
			trace_id: trace_id.unwrap_or_else(gen_trace_id),
			mapped_id,
			metadata: attrs.metadata(),
			name: attrs.metadata().name().into(),
			attributes: Vec::new(),
			start: Some(std::time::SystemTime::now()),
			end: None,
			last_event_time: std::time::Instant::now(),
			active_time: std::time::Duration::default(),
			idle_time: std::time::Duration::default(),
			mapped_parent_id,
			events: Vec::new(),
			links: Vec::new(),
			root: root_child.map(RootNode::Child),
		};

		attrs.record(&mut FieldVisitor(&mut this.attributes, &mut this.name));

		this
	}

	pub fn contains_error(&self) -> bool {
		matches!(self.status, Status::Error { .. })
			|| self.events.iter().any(|e| e.metadata.level() >= &tracing::Level::ERROR)
			|| self.children().any(|c| c.contains_error())
	}

	pub fn children(&self) -> impl Iterator<Item = &SpanNode> {
		match &self.root {
			Some(RootNode::Root(children)) => children.iter(),
			_ => [].iter(),
		}
	}

	pub fn root_id(&self) -> Option<&span::Id> {
		match &self.root {
			Some(RootNode::Root(_)) => Some(&self.id),
			Some(RootNode::Child(id)) => Some(id),
			None => None,
		}
	}

	pub fn event(&mut self, event: &tracing::Event<'_>) {
		let mut attributes = Vec::new();
		let mut name = event.metadata().name().into();
		event.record(&mut FieldVisitor(&mut attributes, &mut name));
		self.events.push(SpanEvent {
			name,
			metadata: event.metadata(),
			time: std::time::SystemTime::now(),
			attributes,
		});
	}

	pub fn record(&mut self, record: &span::Record<'_>) {
		record.record(&mut FieldVisitor(&mut self.attributes, &mut self.name));
	}

	pub fn follows_from(&mut self, id: SpanId, span: Option<&SpanNode>) {
		self.links.push(SpanContext::new(
			span.map_or(TraceId::INVALID, |s| s.trace_id),
			id,
			span.map_or(TraceFlags::NOT_SAMPLED, |_| TraceFlags::SAMPLED),
			false,
			TraceState::NONE,
		));
	}

	pub fn follows_from_context(&mut self, context: SpanContext) {
		self.links.push(context);
	}

	pub fn close(&mut self) {
		self.end = Some(std::time::SystemTime::now());
	}

	pub fn enter(&mut self) {
		self.idle_time += self.last_event_time.elapsed();
		self.last_event_time = std::time::Instant::now();
	}

	pub fn exit(&mut self) {
		self.active_time += self.last_event_time.elapsed();
		self.last_event_time = std::time::Instant::now();
	}

	pub fn trace_id(&self) -> TraceId {
		self.trace_id
	}

	pub fn parent(&self) -> Option<SpanId> {
		self.mapped_parent_id
	}

	pub fn is_root(&self) -> bool {
		matches!(self.root, Some(RootNode::Root(_)))
	}

	pub fn is_child(&self) -> bool {
		matches!(self.root, Some(RootNode::Child(_)))
	}

	pub fn make_root(&mut self) {
		if self.is_root() {
			return;
		}

		if let Some(parent_id) = self.mapped_parent_id.take() {
			self.links.push(SpanContext::new(
				self.trace_id,
				parent_id,
				TraceFlags::SAMPLED,
				false,
				TraceState::NONE,
			));

			// Since we are making this a root span
			// We will need a new trace id
			self.trace_id = gen_trace_id();
		}

		self.root = Some(RootNode::Root(Vec::new()));
	}

	pub fn set_status(&mut self, status: Status) {
		self.status = status;
	}

	pub fn add_child(&mut self, child: SpanNode) {
		if let Some(RootNode::Root(children)) = &mut self.root {
			children.push(child);
		}
	}

	pub fn clear_children(&mut self) {
		if let Some(RootNode::Root(children)) = &mut self.root {
			children.clear();
		}
	}

	pub fn span_count(&self) -> usize {
		match &self.root {
			Some(RootNode::Root(children)) => children.len() + 1,
			_ => 1,
		}
	}

	pub fn flatten(mut self) -> impl Iterator<Item = SpanNode> {
		let children = match self.root.take() {
			Some(RootNode::Root(children)) => Some(children),
			_ => None,
		}
		.into_iter()
		.flatten();

		std::iter::once(self).chain(children)
	}

	pub fn into_data(mut self) -> SpanData {
		static DEFAULT_SPAN: Lazy<SpanData> = Lazy::new(|| SpanData {
			start_time: std::time::SystemTime::UNIX_EPOCH,
			end_time: std::time::SystemTime::UNIX_EPOCH,
			dropped_attributes_count: 0,
			name: Cow::Borrowed(""),
			status: Default::default(),
			instrumentation_lib: Default::default(),
			events: SpanEvents::default(),
			links: Default::default(),
			span_kind: SpanKind::Internal,
			attributes: Vec::new(),
			parent_span_id: SpanId::INVALID,
			span_context: SpanContext::new(
				TraceId::INVALID,
				SpanId::INVALID,
				TraceFlags::default(),
				false,
				TraceState::NONE,
			),
		});

		self.attributes
			.push(KeyValue::new("busy_ns", self.active_time.as_nanos() as i64));
		self.attributes
			.push(KeyValue::new("idle_ns", self.idle_time.as_nanos() as i64));
		if let Some(file) = self.metadata.file() {
			self.attributes.push(KeyValue::new("code.filepath", file))
		}
		if let Some(line) = self.metadata.line() {
			self.attributes.push(KeyValue::new("code.lineno", line as i64))
		}
		if let Some(module) = self.metadata.module_path() {
			self.attributes.push(KeyValue::new("code.namespace", module))
		}
		self.attributes.push(KeyValue::new("level", self.metadata.level().as_str()));

		let mut span = DEFAULT_SPAN.clone();

		span.status = self.status;
		span.start_time = self.start.unwrap();
		span.end_time = self.end.unwrap();
		span.dropped_attributes_count = 0;
		span.name = self.name;
		span.attributes = self.attributes;
		span.parent_span_id = self.mapped_parent_id.unwrap_or(SpanId::INVALID);
		span.span_context = SpanContext::new(self.trace_id, self.mapped_id, TraceFlags::SAMPLED, false, TraceState::NONE);
		span.events.events = self.events.into_iter().map(|e| e.into_data()).collect();
		span.links.links = self.links.into_iter().map(|link| Link::new(link, Vec::new(), 0)).collect();

		span
	}
}

struct FieldVisitor<'a>(&'a mut Vec<opentelemetry::KeyValue>, &'a mut Cow<'static, str>);

impl FieldVisitor<'_> {
	fn push(&mut self, name: &'static str, value: String) {
		if name == "name" {
			*self.1 = format!("{}::{}", self.1, value).into();
		} else {
			self.0.push(KeyValue::new(name, value));
		}
	}
}

impl tracing::field::Visit for FieldVisitor<'_> {
	fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
		self.push(field.name(), value.to_string());
	}

	fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
		self.push(field.name(), format!("{:?}", value));
	}

	fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
		self.push(field.name(), value.to_string());
	}

	fn record_error(&mut self, field: &tracing::field::Field, value: &(dyn std::error::Error + 'static)) {
		self.push(field.name(), value.to_string());
	}

	fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
		self.push(field.name(), value.to_string());
	}

	fn record_i128(&mut self, field: &tracing::field::Field, value: i128) {
		self.push(field.name(), value.to_string());
	}

	fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
		self.push(field.name(), value.to_string());
	}

	fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
		self.push(field.name(), value.to_string());
	}

	fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
		self.push(field.name(), value.to_string());
	}
}
