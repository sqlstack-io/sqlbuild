//! Python's `types_equal` over the native type system.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::clickhouse_type_match::clickhouse_type_match;
use crate::analysis_route::models::AnalysisRoute;
use crate::contracts::constants::{CLICKHOUSE_DIALECT, UPPER_FOLDING_DIALECT};
use crate::contracts::models::{DeclaredColumn, InferredOutputColumn};
use crate::type_system::errors::TypeNormalizationError;
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::{NormalizedType, TypeNormalization};

/// Whether two type strings normalize to the same comparison shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeComparison {
    Equal,
    Different,
}

/// One request's type comparisons, normalizing each type string once as Python's cache does.
pub(crate) struct TypeComparer<'a> {
    dialect: &'a str,
    /// ClickHouse on its native route, whose types are compared without top-level wrappers.
    native_clickhouse: bool,
    normalized: RefCell<HashMap<String, Result<NormalizedType, TypeNormalizationError>>>,
}

impl<'a> TypeComparer<'a> {
    /// Whether the dialect folds unquoted identifiers to upper case, as star expansion names them.
    pub(crate) fn folds_upper(&self) -> bool {
        self.dialect == UPPER_FOLDING_DIALECT
    }

    pub(crate) fn new(dialect: &'a str) -> Self {
        Self {
            dialect,
            native_clickhouse: dialect == CLICKHOUSE_DIALECT
                && matches!(analysis_route(Some(dialect)), AnalysisRoute::Native(_)),
            normalized: RefCell::new(HashMap::new()),
        }
    }

    /// Compare two types as Python's `types_equal`, or Python's normalization error.
    pub(crate) fn types_equal(
        &self,
        left: &str,
        right: &str,
    ) -> Result<TypeComparison, TypeNormalizationError> {
        if self.native_clickhouse {
            return Ok(if clickhouse_type_match(left, right, false).same_type {
                TypeComparison::Equal
            } else {
                TypeComparison::Different
            });
        }
        let left: NormalizedType = self.normalized(left)?;
        let right: NormalizedType = self.normalized(right)?;
        Ok(if left == right {
            TypeComparison::Equal
        } else {
            TypeComparison::Different
        })
    }

    /// Whether a column that may be null meets a declaration that does not allow it.
    pub(crate) fn nullable_into_non_null(
        &self,
        declared: &DeclaredColumn,
        inferred: &InferredOutputColumn,
    ) -> bool {
        if declared.not_null && inferred.nullable {
            return true;
        }
        let (true, Some(declared_type), Some(inferred_type)) = (
            self.native_clickhouse,
            declared.declared_type.as_deref(),
            inferred.inferred_type.as_deref(),
        ) else {
            return false;
        };
        clickhouse_type_match(declared_type, inferred_type, inferred.nullable)
            .nullable_into_non_null
    }

    fn normalized(&self, type_sql: &str) -> Result<NormalizedType, TypeNormalizationError> {
        if let Some(known) = self.normalized.borrow().get(type_sql) {
            return known.clone();
        }
        let normalized: Result<NormalizedType, TypeNormalizationError> =
            normalize_type(type_sql, self.dialect)
                .map(|TypeNormalization { normalized, .. }| normalized);
        self.normalized
            .borrow_mut()
            .insert(type_sql.to_owned(), normalized.clone());
        normalized
    }
}
