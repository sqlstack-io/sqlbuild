//! Python's `types_equal` over the native type system.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::{AnalysisRoute, TypeMatch};
use crate::analysis_route::types::NativeDialect;
use crate::contracts::constants::UPPER_FOLDING_DIALECT;
use crate::contracts::models::{DeclaredColumn, InferredOutputColumn};
use crate::type_system::errors::TypeNormalizationError;
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::{NormalizedType, TypeNormalization};

/// Whether two type strings normalize to the same comparison shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeComparison {
    Equal,
    Different,
    /// A native route cannot spell one of the types, or has no parser: no finding either way.
    NotCompared,
}

/// One request's type comparisons, normalizing each type string once as Python's cache does.
pub(crate) struct TypeComparer<'a> {
    dialect: &'a str,
    /// The dialect's route: a native module compares types itself, and no route compares none.
    route: AnalysisRoute,
    normalized: RefCell<HashMap<String, Result<NormalizedType, TypeNormalizationError>>>,
    /// A native route's matches by declared type, inferred type and its nullability.
    native_matches: RefCell<HashMap<NativeMatchKey, TypeMatch>>,
}

type NativeMatchKey = (String, Option<String>, bool);

impl<'a> TypeComparer<'a> {
    /// Whether the dialect folds unquoted identifiers to upper case, as star expansion names them.
    pub(crate) fn folds_upper(&self) -> bool {
        self.dialect == UPPER_FOLDING_DIALECT
    }

    pub(crate) fn new(dialect: &'a str) -> Self {
        Self {
            dialect,
            route: analysis_route(Some(dialect)),
            normalized: RefCell::new(HashMap::new()),
            native_matches: RefCell::new(HashMap::new()),
        }
    }

    /// Compare two types as Python's `types_equal`, or Python's normalization error.
    pub(crate) fn types_equal(
        &self,
        left: &str,
        right: &str,
    ) -> Result<TypeComparison, TypeNormalizationError> {
        match self.route {
            AnalysisRoute::Legacy => {}
            AnalysisRoute::Unavailable => return Ok(TypeComparison::NotCompared),
            AnalysisRoute::Native(dialect) => {
                return Ok(match self.native_match(dialect, left, Some(right), false) {
                    TypeMatch::NotCompared { .. } => TypeComparison::NotCompared,
                    TypeMatch::Compared {
                        same_type: true, ..
                    } => TypeComparison::Equal,
                    TypeMatch::Compared { .. } => TypeComparison::Different,
                });
            }
        }
        let left: NormalizedType = self.normalized(left)?;
        let right: NormalizedType = self.normalized(right)?;
        Ok(if left == right {
            TypeComparison::Equal
        } else {
            TypeComparison::Different
        })
    }

    /// Whether casting a value of `inferred` type to `declared` keeps it, so a casting build stores it.
    pub(crate) fn casts_losslessly(&self, declared: &str, inferred: &str) -> bool {
        let AnalysisRoute::Native(dialect) = self.route else {
            return false;
        };
        match (
            dialect.normalize_type(declared),
            dialect.normalize_type(inferred),
        ) {
            (Some(declared), Some(inferred)) => dialect.casts_losslessly(&declared, &inferred),
            _ => false,
        }
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
        let (AnalysisRoute::Native(dialect), Some(declared_type)) =
            (self.route, declared.declared_type.as_deref())
        else {
            return false;
        };
        matches!(
            self.native_match(
                dialect,
                declared_type,
                inferred.inferred_type.as_deref(),
                inferred.nullable
            ),
            TypeMatch::Compared {
                nullable_into_non_null: true,
                ..
            } | TypeMatch::NotCompared {
                nullable_into_non_null: true
            }
        )
    }

    /// `dialect.type_match`, computed once per declared type, inferred type and nullability.
    fn native_match(
        &self,
        dialect: &dyn NativeDialect,
        declared: &str,
        inferred: Option<&str>,
        inferred_nullable: bool,
    ) -> TypeMatch {
        let key: NativeMatchKey = (
            declared.to_owned(),
            inferred.map(str::to_owned),
            inferred_nullable,
        );
        if let Some(found) = self.native_matches.borrow().get(&key) {
            return *found;
        }
        let found: TypeMatch = dialect.type_match(declared, inferred, inferred_nullable);
        self.native_matches.borrow_mut().insert(key, found);
        found
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
