//! Public constants of the dbt event stream.

use crate::dbt_events::_helpers::getters::read_dbt_event_stream_update;
use crate::dbt_events::models::DbtEventStreamUpdate;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<DbtEventStreamUpdate>,
    read: read_dbt_event_stream_update,
}];

/// The keywords `DbtEventStream` accepts: required, then optional.
pub(crate) const STREAM_OPTIONS: (&[&str], &[&str]) =
    (&["display_total", "detail_by_unique_id", "use_color"], &[]);
/// The keywords `render_dbt_node_result` accepts: required, then optional.
pub(crate) const RENDER_OPTIONS: (&[&str], &[&str]) = (
    &["result", "use_color"],
    &["display_index", "display_total", "detail"],
);
