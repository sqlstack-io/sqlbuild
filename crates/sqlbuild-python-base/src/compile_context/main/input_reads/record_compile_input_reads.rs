//! Record template input reads from every thread until the returned block exits.

use crate::compile_context::_helpers::input_reads;
use crate::compile_context::models::CompileInputRecording;

/// Record template input reads from every thread until the returned block exits.
#[inline]
pub fn record_compile_input_reads() -> CompileInputRecording {
    input_reads::record_compile_input_reads()
}
