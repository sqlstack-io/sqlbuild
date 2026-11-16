//! Public constants of input loops.

use crate::input_loops::_helpers::resources::resource_attachment::attach_audits;
use crate::input_loops::_helpers::resources::resource_attachment::attach_functions;
use crate::input_loops::_helpers::resources::resource_attachment::attach_sources;
use crate::input_loops::_helpers::resources::resource_attachment::attach_sql_tests;
use crate::input_loops::_helpers::resources::resource_diagnostics::resource_diagnostics;
use sqlbuild_python_project::compiled_project::models::ResourceAttachment;

/// The attachment runs the project's Python methods dispatch to, installed at registration.
pub const RESOURCE_ATTACHMENT: ResourceAttachment = ResourceAttachment {
    sources: attach_sources,
    functions: attach_functions,
    diagnostics: resource_diagnostics,
    sql_tests: attach_sql_tests,
    audits: attach_audits,
};
