//! Each native condition consumes its bounded page inside the evaluation transaction.

use uuid::Uuid;

use crate::sensors::_helpers::nodes::consume_node_events;
use crate::sensors::_helpers::platform::consume_platform_events;
use crate::sensors::_helpers::runs::consume_run_events;
use crate::sensors::models::{SensorCondition, SensorCursor, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn consume_condition(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    match &evaluation.definition.when {
        SensorCondition::Custom { .. } => Err(ControlError::Invalid(
            "custom sensors require a location result".into(),
        )),
        SensorCondition::PlatformEvent { kinds } => {
            consume_platform_events(transaction, evaluation, kinds).await
        }
        SensorCondition::NodeEvent { selection, kinds } => {
            consume_node_events(transaction, evaluation, selection, kinds).await
        }
        SensorCondition::RunEvent {
            job_key,
            kinds,
            executed_successfully,
        } => {
            consume_run_events(
                transaction,
                evaluation,
                (job_key, kinds),
                executed_successfully.as_ref(),
            )
            .await
        }
    }
}
