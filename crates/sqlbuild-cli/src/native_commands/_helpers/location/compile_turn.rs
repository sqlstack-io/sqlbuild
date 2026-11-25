//! One compile turn of a serving location: the next generation number, its compile, and what settles it.

use std::future::Future;
use std::path::Path;

use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::{mpsc, watch};

use crate::native_commands::_helpers::location::execution::{
    compile_next_generation, finish_compile,
};
use crate::native_commands::_helpers::location::lookup_failure::failed_lookup;
use crate::native_commands::models::{
    DevControlState, LocationRuntimeRequest, LocationServe, LocationWatch,
};
use crate::native_commands::types::{CompileState, LocationSession};

/// Compile the next generation and settle it, with the generation it published; `None` when stopped. A failed lookup retries in 5 seconds and reports a failed load at the third in a row.
pub(crate) async fn compile_turn(
    ((options, script), (bearer, client, poll), requests): (
        (&LocationServe, &Path),
        LocationSession<'_>,
        &mpsc::Sender<LocationRuntimeRequest>,
    ),
    ((watch, metadata, deferred_sync), compiled_inputs): CompileState,
    (controls, stopped): (
        &mut watch::Receiver<DevControlState>,
        impl Future<Output = Result<(), ControlError>>,
    ),
) -> Result<Option<(CompileState, Option<i64>)>, ControlError> {
    tokio::pin!(stopped);
    let Some((next_compile, metadata, compiled_inputs)) = compile_next_generation(
        (options, bearer, requests),
        (metadata, compiled_inputs, &watch.observed.0),
        stopped.as_mut(),
    )
    .await?
    else {
        return Ok(None);
    };
    let (load, generation) = match next_compile {
        Ok(next) => next,
        Err(error) => {
            let retried: Option<LocationWatch> =
                failed_lookup(options, (bearer, poll.session_id), (&error, watch), stopped).await?;
            return Ok(
                retried.map(|watch| (((watch, metadata, deferred_sync), compiled_inputs), None))
            );
        }
    };
    let mut watch = watch;
    watch.lookups.answered(generation);
    let published: Option<i64> = match &load {
        Ok(_) => Some(generation),
        Err(_) => None,
    };
    let settled = finish_compile(
        (options, script, ((bearer, poll.session_id), client)),
        (load, generation),
        (watch, metadata, deferred_sync),
        (controls, stopped.as_mut()),
    )
    .await?;
    Ok(settled.map(|(watch, metadata, deferred_sync)| {
        (
            ((watch, metadata, deferred_sync), compiled_inputs),
            published,
        )
    }))
}
