use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use crate::models::{EvaluateRequest, Fault, Model, RuleMetadata, RulesConfig};
use crate::rules::types::ShapeFacts;
use globset::GlobSet;

/// The query shapes one rules evaluation has parsed, keyed by dialect and byte-identical SQL.
#[derive(Debug, Default)]
pub(crate) struct QueryShapes {
    entries: Mutex<HashMap<(String, String), ShapeFacts>>,
}

impl QueryShapes {
    pub(crate) fn entry(&self, dialect: &str, sql: &str) -> ShapeFacts {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = entries.get(&(dialect.to_owned(), sql.to_owned())) {
            return Arc::clone(entry);
        }
        let entry: ShapeFacts = Arc::default();
        entries.insert((dialect.to_owned(), sql.to_owned()), Arc::clone(&entry));
        entry
    }
}

#[derive(Debug, Default)]
pub(crate) struct FaultCollector {
    faults: RefCell<Vec<Fault>>,
}

#[derive(Debug)]
pub(crate) struct ModelEvaluationRequest<'a> {
    pub(crate) model: &'a Model,
    pub(crate) config: &'a RulesConfig,
    pub(crate) selected: &'a BTreeMap<String, &'a RuleMetadata>,
    pub(crate) request: &'a EvaluateRequest,
    pub(crate) threshold_overrides: &'a [ResolvedThresholdOverride],
    pub(crate) shapes: &'a QueryShapes,
}

#[derive(Debug)]
pub(crate) struct ProjectEvaluationRequest<'a> {
    pub(crate) selected: &'a BTreeMap<String, &'a RuleMetadata>,
    pub(crate) request: &'a EvaluateRequest,
}

#[derive(Debug)]
pub(crate) struct ResolvedThresholdOverride {
    pub(crate) matcher: GlobSet,
    pub(crate) thresholds: BTreeMap<String, u32>,
}

impl ResolvedThresholdOverride {
    pub(crate) fn matches(&self, path: &str) -> bool {
        self.matcher.is_match(path)
    }

    pub(crate) fn threshold(&self, name: &str) -> Option<u32> {
        self.thresholds.get(name).copied()
    }
}

impl FaultCollector {
    pub(crate) fn push(&self, fault: Fault) {
        self.faults.borrow_mut().push(fault);
    }

    pub(crate) fn into_inner(self) -> Vec<Fault> {
        self.faults.into_inner()
    }
}

/// The value kind of one built-in rule option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinOptionKind {
    /// A boolean defaulting to false.
    Boolean,
    /// A list of strings with the given default.
    StringList(&'static [&'static str]),
    /// A list of tables, each checked by the rule's own configuration validation.
    TableList,
    /// A whole number of at least `minimum`, with the given default.
    Integer { default: u32, minimum: u32 },
}

/// One option a built-in rule declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinOption {
    pub kind: BuiltinOptionKind,
    pub name: &'static str,
    pub description: &'static str,
}
