//! Public constants of result output.

use crate::result_output::_helpers::events::integration_envelope::IntegrationAssetResult;
use crate::result_output::_helpers::events::integration_envelope::IntegrationCheckResult;
use crate::result_output::_helpers::events::integration_envelope::IntegrationResultEnvelope;
use crate::result_output::_helpers::events::terminal_index::TerminalEventClaim;
use crate::result_output::_helpers::results::getters::read_integration_asset_result;
use crate::result_output::_helpers::results::getters::read_integration_check_result;
use crate::result_output::_helpers::results::getters::read_integration_result_envelope;
use crate::result_output::_helpers::results::getters::read_terminal_event_claim;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

pub const RESULT_PATH_ENV: &str = "SQLBUILD_INTEGRATION_RESULT_PATH";

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<IntegrationAssetResult>,
        read: read_integration_asset_result,
    },
    GetterEntry {
        class: class_of::<IntegrationCheckResult>,
        read: read_integration_check_result,
    },
    GetterEntry {
        class: class_of::<IntegrationResultEnvelope>,
        read: read_integration_result_envelope,
    },
    GetterEntry {
        class: class_of::<TerminalEventClaim>,
        read: read_terminal_event_claim,
    },
];
