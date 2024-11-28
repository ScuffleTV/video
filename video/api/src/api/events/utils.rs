use ulid::Ulid;

pub fn ack_key(organization_id: Ulid, event_id: Ulid) -> String {
	format!("events_ack:{organization_id}:{event_id}")
}
