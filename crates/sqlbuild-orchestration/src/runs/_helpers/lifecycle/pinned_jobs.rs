//! A narrowed run's pinned jobs keep only its own nodes, and still give it their build-wide settings.

use crate::runs::models::RunRequest;

/// Keep each pinned job's nodes that `request` still selects; a job left with none stays named.
pub(crate) fn narrow_pinned_jobs(request: &mut RunRequest) {
    for job in request.jobs.values_mut() {
        job.selection.retain(|key| request.selection.contains(key));
    }
}
