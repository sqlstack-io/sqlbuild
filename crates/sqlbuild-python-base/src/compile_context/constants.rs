//! Public constants of compile context.

use crate::compile_context::_helpers::getters::read_collected_compile_diagnostics;
use crate::compile_context::_helpers::getters::read_compile_input_reads;
use crate::compile_context::_helpers::input_reads::CompileInputReads;
use crate::compile_context::models::CollectedCompileDiagnostics;
use crate::records::main::registry::class_of::class_of;
use crate::records::models::GetterEntry;

/// Whether edits that report or raise errors compile as deltas; until then they fall back.
pub const ERRORS_ON_DELTA_PATH: bool = true;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<CollectedCompileDiagnostics>,
        read: read_collected_compile_diagnostics,
    },
    GetterEntry {
        class: class_of::<CompileInputReads>,
        read: read_compile_input_reads,
    },
];
