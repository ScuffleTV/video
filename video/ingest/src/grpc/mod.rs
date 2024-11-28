use std::sync::Arc;

use tonic::transport::server::Router;

use crate::global::IngestGlobal;

mod ingest;

pub fn add_routes<G: IngestGlobal, L>(global: &Arc<G>, router: Router<L>) -> Router<L> {
	router.add_service(ingest::IngestServer::new(global))
}
