//! Cases for the limits PostgreSQL holds the server's sessions to.

use crate::health::models::ControlCauseCase;
use crate::store::models::TransactionMode;

#[derive(Debug)]
pub(super) struct SessionLimitTestCase {
    pub(super) description: &'static str,
    pub(super) mode: TransactionMode,
    /// `SHOW statement_timeout` and `SHOW idle_in_transaction_session_timeout` inside the transaction.
    pub(super) expected_limits: (&'static str, &'static str),
}

#[derive(Debug)]
pub(super) struct CutOffTestCase {
    pub(super) description: &'static str,
    /// The statement that lowers one limit for the transaction, the pause after it, and the statement PostgreSQL cuts off.
    pub(super) session: (&'static str, u64, &'static str),
    pub(super) expected_report: CutOffReport,
}

/// How a cut-off session was reported: the failed statement's cause, whether the probe's report found it, the control database's health afterwards and each stored platform event's `idle`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CutOffReport {
    pub(super) cause: Option<ControlCauseCase>,
    pub(super) reported: bool,
    pub(super) health: Option<ControlCauseCase>,
    pub(super) events: Vec<bool>,
}
