//! Removing one run's output so the index never points at a missing object.

use futures_util::StreamExt;
use object_store::ObjectStoreExt;
use uuid::Uuid;

use crate::run_output::_helpers::objects::run_prefix;
use crate::run_output::models::OutputStore;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::write::write;
use crate::store::models::{ControlStore, Parameter};

/// The index of the run's output goes first, keeping only its `sqb_run_output` row; then its
/// objects; then that row. A failure part-way leaves the row, so the next pass tries again, and
/// nothing is left that points at a deleted object.
pub(crate) async fn prune_run(
    store: &ControlStore,
    output: &OutputStore,
    run_id: Uuid,
) -> Result<(), ControlError> {
    delete_index(
        store,
        run_id,
        &[
            "DELETE FROM sqb_run_output_marks WHERE run_id=$1",
            "DELETE FROM sqb_run_output_segments WHERE run_id=$1",
            "DELETE FROM sqb_run_output_chunks WHERE run_id=$1",
            "DELETE FROM sqb_run_output_streams WHERE run_id=$1",
        ],
    )
    .await?;
    let prefix = run_prefix(output, run_id);
    let mut listed = output.objects.list(Some(&prefix));
    while let Some(object) = listed.next().await {
        output.objects.delete(&object?.location).await?;
    }
    delete_index(store, run_id, &["DELETE FROM sqb_run_output WHERE run_id=$1"]).await
}

async fn delete_index(
    store: &ControlStore,
    run_id: Uuid,
    statements: &[&str],
) -> Result<(), ControlError> {
    write(store, |mut transaction| async move {
        for sql in statements {
            (transaction, _) = query(transaction, sql, vec![Parameter::Id(run_id)]).await?;
        }
        Ok((transaction, ()))
    })
    .await
}
