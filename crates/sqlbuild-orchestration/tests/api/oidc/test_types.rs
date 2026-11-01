//! Disposable HTTPS identity-provider state; signing keys are generated for each fixture.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use openidconnect::core::CoreEdDsaPrivateSigningKey;

#[derive(Clone)]
pub(crate) struct ProviderState {
    pub(crate) issuer: String,
    pub(crate) keys: Arc<BTreeMap<String, Arc<CoreEdDsaPrivateSigningKey>>>,
    pub(crate) published_key: Arc<Mutex<String>>,
    pub(crate) codes: Arc<Mutex<BTreeMap<String, BTreeMap<String, String>>>>,
    pub(crate) fault: Arc<Mutex<ProviderFault>>,
    pub(crate) subject: Arc<Mutex<String>>,
}

pub(crate) struct TestProvider {
    pub(crate) state: ProviderState,
    pub(crate) authority: std::path::PathBuf,
    pub(crate) client: reqwest::Client,
    pub(crate) handle: axum_server::Handle<std::net::SocketAddr>,
    pub(crate) task: tokio::task::JoinHandle<std::io::Result<()>>,
    pub(crate) directory: tempfile::TempDir,
}

pub(crate) struct BrowserFlow {
    pub(crate) cookie: String,
    pub(crate) callback: reqwest::Url,
}

#[derive(Clone, Default, serde::Deserialize)]
#[serde(default)]
pub(crate) struct ProviderFault {
    pub(crate) claims: serde_json::Map<String, serde_json::Value>,
    pub(crate) response: serde_json::Map<String, serde_json::Value>,
    pub(crate) remove_fields: Vec<String>,
    pub(crate) signing_key: Option<String>,
}
