//! Facts one authored string's macro expansion records, in encounter order.

use std::collections::HashSet;

use pyo3::prelude::{Py, PyAny};
use sqlbuild_render::macro_calls::models::MacroCallEvent;

/// A typed resource reference by its `SqlResourceRefKind` value and name.
pub(crate) type ResourceRef = (String, String);

/// `MacroExpansionFacts`: dependencies, usages, relations, call-site references and the event log.
#[derive(Debug, Default)]
pub(crate) struct ExpansionFacts {
    pub(crate) dependencies: Vec<Py<PyAny>>,
    pub(crate) usages: Vec<Py<PyAny>>,
    /// The string's relation table: each typed reference a macro rendered, by placeholder index.
    pub(crate) relations: Vec<ResourceRef>,
    pub(crate) argument_references: Vec<ResourceRef>,
    call_site_refs: Vec<HashSet<ResourceRef>>,
    rendering_approved: Vec<HashSet<ResourceRef>>,
    event_log: Option<Vec<MacroCallEvent>>,
    memo_blocked: bool,
}

impl ExpansionFacts {
    /// Start recording the consumer-independent facts of one top-level macro call.
    pub(crate) fn begin_event_log(&mut self) {
        self.event_log = Some(Vec::new());
        self.memo_blocked = false;
    }

    /// Stop recording; the call's events and whether it may be replayed.
    pub(crate) fn end_event_log(&mut self) -> (Vec<MacroCallEvent>, bool) {
        (
            self.event_log.take().unwrap_or_default(),
            !self.memo_blocked,
        )
    }

    /// Record one fact of the executing call, when recording.
    pub(crate) fn note_event(&mut self, event: MacroCallEvent) {
        if let Some(log) = self.event_log.as_mut() {
            log.push(event);
        }
    }

    /// Mark the executing call as dependent on its consumer, so it is never replayed.
    pub(crate) fn block_memo(&mut self) {
        self.memo_blocked = true;
    }

    /// Record references written as arguments of every macro call being expanded.
    pub(crate) fn record_call_site_refs(&mut self, refs: &[ResourceRef]) {
        for (kind, name) in refs {
            self.note_event(MacroCallEvent::ArgumentReference {
                kind: kind.clone(),
                name: name.clone(),
            });
        }
        for reference in refs {
            if !self.argument_references.contains(reference) {
                self.argument_references.push(reference.clone());
            }
        }
        for approved in &mut self.call_site_refs {
            approved.extend(refs.iter().cloned());
        }
    }

    /// Start collecting the references written as one macro call's arguments.
    pub(crate) fn open_call_site(&mut self) {
        self.call_site_refs.push(HashSet::new());
    }

    /// Stop collecting for the innermost call and return its argument references.
    pub(crate) fn close_call_site(&mut self) -> HashSet<ResourceRef> {
        self.call_site_refs.pop().unwrap_or_default()
    }

    /// Approve the call-site argument references of the macro about to run.
    pub(crate) fn begin_rendering(&mut self, approved: HashSet<ResourceRef>) {
        self.rendering_approved.push(approved);
    }

    /// Restore the approvals of the enclosing macro after one macro returns.
    pub(crate) fn end_rendering(&mut self) {
        let _ = self.rendering_approved.pop();
    }

    /// The placeholder index of an approved reference, or `None` when it renders as its call.
    pub(crate) fn relation_index(&mut self, reference: &ResourceRef) -> Option<usize> {
        if !self
            .rendering_approved
            .last()
            .is_some_and(|approved| approved.contains(reference))
        {
            return None;
        }
        Some(self.relation_slot(reference))
    }

    /// `relations.setdefault(reference, len(relations))`.
    pub(crate) fn relation_slot(&mut self, reference: &ResourceRef) -> usize {
        if let Some(index) = self.relations.iter().position(|known| known == reference) {
            return index;
        }
        self.relations.push(reference.clone());
        self.relations.len() - 1
    }
}
