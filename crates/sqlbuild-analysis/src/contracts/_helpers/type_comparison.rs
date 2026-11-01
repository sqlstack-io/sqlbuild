//! Declared-versus-inferred type comparison over the native type system: Python's `types_equal`,
//! plus lossless NUMBER widening (D31).

use std::cell::RefCell;
use std::collections::HashMap;

use crate::contracts::constants::UPPER_FOLDING_DIALECT;
use crate::type_system::errors::TypeNormalizationError;
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::{NormalizedType, PythonInteger, TypeFamily, TypeNormalization};

/// Whether a declared type accepts an inferred one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeComparison {
    Equal,
    Different,
}

/// One request's type comparisons, normalizing each type string once as Python's cache does.
pub(crate) struct TypeComparer<'a> {
    dialect: &'a str,
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
            normalized: RefCell::new(HashMap::new()),
        }
    }

    /// Whether a declared type accepts an inferred one: equal types, or a lossless NUMBER
    /// widening, `NUMBER(q,s)` into `NUMBER(p,s)` with `q <= p` and the same scale.
    pub(crate) fn declared_accepts(
        &self,
        declared: &str,
        inferred: &str,
    ) -> Result<TypeComparison, TypeNormalizationError> {
        let declared: NormalizedType = self.normalized(declared)?;
        let inferred: NormalizedType = self.normalized(inferred)?;
        Ok(
            if declared == inferred || widens_losslessly(&inferred, &declared) {
                TypeComparison::Equal
            } else {
                TypeComparison::Different
            },
        )
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

/// Whether every `narrow` value fits `wide` exactly: decimals of one scale, `narrow` no more precise.
fn widens_losslessly(narrow: &NormalizedType, wide: &NormalizedType) -> bool {
    let digits = |value: &Option<PythonInteger>| -> Option<u64> {
        value
            .as_ref()
            .and_then(|value| value.as_str().parse::<u64>().ok())
    };
    narrow.family == TypeFamily::Decimal
        && wide.family == TypeFamily::Decimal
        && narrow.scale.is_some()
        && narrow.scale == wide.scale
        && matches!(
            (digits(&narrow.precision), digits(&wide.precision)),
            (Some(narrow), Some(wide)) if narrow <= wide
        )
}
