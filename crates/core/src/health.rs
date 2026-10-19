//! Health port: lets the HTTP layer ask whether the database is reachable
//! without knowing which adapter backs it (NFR-24).

use std::future::Future;

/// Reports whether the backing database can currently be reached.
pub trait HealthProbe: Send + Sync + 'static {
    fn database_reachable(&self) -> impl Future<Output = bool> + Send;
}
