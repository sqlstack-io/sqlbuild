//! Visited nodes kept in first-visit order.

use std::collections::HashSet;
use std::convert::Infallible;
use std::hash::Hash;

use crate::graph_walk::types::NodeSet;

/// Caller-owned selection and materialization predicates, and the ancestor kind to collect.
#[derive(Debug)]
pub struct ClonePolicy<P, C, V> {
    pub selected: P,
    pub clonable: C,
    pub view: V,
    pub skipped_views: bool,
}

/// Distinct nodes in the order a walk first recorded them.
#[derive(Debug, Clone)]
pub struct VisitOrder<K> {
    /// The recorded nodes, first visit first.
    pub order: Vec<K>,
    seen: HashSet<K>,
}

impl<K> Default for VisitOrder<K> {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            seen: HashSet::new(),
        }
    }
}

impl<K: Eq + Hash + Clone> NodeSet<K> for VisitOrder<K> {
    type Error = Infallible;

    fn insert_new(&mut self, key: &K) -> Result<bool, Infallible> {
        if !self.seen.insert(key.clone()) {
            return Ok(false);
        }
        self.order.push(key.clone());
        Ok(true)
    }

    fn holds(&self, key: &K) -> Result<bool, Infallible> {
        Ok(self.seen.contains(key))
    }
}
