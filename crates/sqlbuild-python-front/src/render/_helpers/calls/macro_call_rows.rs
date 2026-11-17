//! Python rows of macro call sites and splice spans.

use sqlbuild_python_base::positional_record;
use sqlbuild_render::macro_calls::models::MacroCallSite;

positional_record! {
    into;
    /// One call site: start, arguments start, end, name, tree names if scanned and whether typed reference text appears.
    pub(crate) struct SiteRow {
        start: usize,
        args_start: usize,
        end: usize,
        name: String,
        tree_names: Option<Vec<String>>,
        typed_reference_text: bool,
    }
}

pub(crate) fn site_row(site: MacroCallSite) -> SiteRow {
    SiteRow {
        start: site.start,
        args_start: site.args_start,
        end: site.end,
        name: site.name,
        tree_names: site.tree_names,
        typed_reference_text: site.typed_reference_text,
    }
}
