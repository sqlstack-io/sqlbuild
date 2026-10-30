//! A worker owns only an API credential, never a control-store connection.

use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct WorkerClient {
    pub(crate) client: reqwest::Client,
    pub(crate) run_url: reqwest::Url,
    pub(crate) run_id: Uuid,
    pub(crate) claim_id: Uuid,
}
