use crate::scope_report::models::ScopeResult;
use crate::scope_report::models::{BrowseResult, ListResult, ScopeFolder, ScopeSection};
use crate::scope_text::models::ScopeTextRequest;

/// The request of `sqb scope orders` with default options.
pub(super) fn orders_request() -> ScopeTextRequest {
    ScopeTextRequest {
        target: Some("model:orders".to_owned()),
        nearby_depth: 1,
        globals: "summary".to_owned(),
        page_size: 100,
        paths: "relative".to_owned(),
        ..ScopeTextRequest::default()
    }
}

/// A browse of `models` with one child folder.
pub(super) fn browse_result(complete: bool) -> ScopeResult {
    ScopeResult::Browse(BrowseResult {
        folder: "models".to_owned(),
        folders: vec![ScopeFolder {
            path: "sql/models/my marts".to_owned(),
            name: "my marts".to_owned(),
            descendant_count: 1,
            child_count: 0,
            used_count: 1,
            kind_counts: vec![("macro".to_owned(), 1)],
        }],
        diagnostics: Vec::new(),
        complete,
    })
}

/// An empty, complete list page with a continuation cursor.
pub(super) fn list_result() -> ScopeResult {
    ScopeResult::List(ListResult {
        folder: ".".to_owned(),
        declarations: Vec::new(),
        section: ScopeSection {
            name: "declarations".to_owned(),
            total: 2,
            returned: 0,
            collapsed: false,
            collapsed_count: 0,
            truncated: true,
            complete: true,
            next_cursor: Some("c1".to_owned()),
            cursor: None,
            page_size: 1,
        },
        diagnostics: Vec::new(),
    })
}
