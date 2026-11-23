//! Start sending this process's run output to the server, on a thread with its own HTTP client.

use std::sync::{Arc, Mutex};

use crate::run_output::_helpers::upload::{attached, send_batches};
use crate::run_output::models::{RunOutputUploader, UploadShared};
use crate::worker::errors::WorkerError;
use crate::worker::models::WorkerClient;

/// Every byte captured from now on is sent under `client`'s claim until `finish_output_upload`.
pub fn start_output_upload(client: &WorkerClient) -> Result<Arc<RunOutputUploader>, WorkerError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let independent = {
        let _entered = runtime.enter();
        WorkerClient {
            connection: client.connection.independent()?,
            ..client.clone()
        }
    };
    let shared: Arc<UploadShared> = Arc::default();
    let sending: Arc<UploadShared> = Arc::clone(&shared);
    let sender = std::thread::Builder::new()
        .name("sqlbuild-run-output".into())
        .spawn(move || send_batches(&sending, &independent, &runtime))?;
    let uploader = Arc::new(RunOutputUploader {
        shared,
        sender: Mutex::new(Some(sender)),
    });
    *attached() = Some(Arc::clone(&uploader));
    Ok(uploader)
}
