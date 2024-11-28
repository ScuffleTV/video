use std::sync::Arc;

use scuffle_image_processor_proto::{event_callback, EventCallback, EventQueue as EventTopic};

use crate::database::Job;
use crate::event_queue::EventQueue;
use crate::global::Global;
use crate::worker::JobError;

#[tracing::instrument(skip(global, job, event_topic), fields(topic = %event_topic.topic, name = %event_topic.name, job_id = %job.id))]
pub async fn on_event(global: &Arc<Global>, job: &Job, event_topic: &EventTopic, event: event_callback::Event) {
	let Some(queue) = global.event_queue(&event_topic.name) else {
		tracing::warn!("event queue not found: {}", event_topic.name);
		return;
	};

	if let Err(err) = queue
		.publish(
			&event_topic.topic,
			EventCallback {
				id: job.id.to_string(),
				timestamp: chrono::Utc::now().timestamp() as u64,
				metadata: job.task.events.as_ref().map(|e| e.metadata.clone()).unwrap_or_default(),
				event: Some(event),
			},
		)
		.await
	{
		tracing::error!("failed to publish event: {err}");
	}
}

pub async fn on_start(global: &Arc<Global>, job: &Job) {
	if let Some(on_start) = &job.task.events.as_ref().and_then(|events| events.on_start.as_ref()) {
		on_event(global, job, on_start, event_callback::Event::Start(event_callback::Start {})).await;
	}
}

pub async fn on_success(global: &Arc<Global>, job: &Job, success: event_callback::Success) {
	if let Some(on_success) = &job.task.events.as_ref().and_then(|events| events.on_success.as_ref()) {
		on_event(global, job, on_success, event_callback::Event::Success(success)).await;
	}
}

pub async fn on_failure(global: &Arc<Global>, job: &Job, err: JobError) {
	if let Some(on_failure) = &job.task.events.as_ref().and_then(|events| events.on_failure.as_ref()) {
		on_event(
			global,
			job,
			on_failure,
			event_callback::Event::Fail(event_callback::Fail { error: Some(err.into()) }),
		)
		.await;
	}
}

pub async fn on_cancel(global: &Arc<Global>, job: &Job) {
	if let Some(on_cancel) = &job.task.events.as_ref().and_then(|events| events.on_cancel.as_ref()) {
		on_event(
			global,
			job,
			on_cancel,
			event_callback::Event::Cancel(event_callback::Cancel {}),
		)
		.await;
	}
}
