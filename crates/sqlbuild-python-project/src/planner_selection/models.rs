//! Public records of planner selection.

use crate::planner_selection::_helpers::carrier::PRIVATE_USE;
use crate::planner_selection::_helpers::carrier::ordinal;
use crate::planner_selection::_helpers::carrier::surrogate_text;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;
use std::collections::BTreeSet;
use std::collections::HashSet;

/// `str.translate` tables to and from carrier characters; empty when no text holds a surrogate.
pub struct Carrier<'py> {
    pub(crate) py: Python<'py>,
    pub(crate) tables: Option<(Bound<'py, PyDict>, Bound<'py, PyDict>)>,
}

impl std::fmt::Debug for Carrier<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Carrier").finish_non_exhaustive()
    }
}

impl<'py> Carrier<'py> {
    /// A carrier for `texts`, choosing carriers that appear in neither `texts` nor `reserved`.
    pub fn new(
        py: Python<'py>,
        texts: &[Bound<'py, PyString>],
        reserved: impl FnOnce() -> PyResult<Vec<String>>,
    ) -> PyResult<Self> {
        let mut surrogates: BTreeSet<u32> = BTreeSet::new();
        let mut taken: HashSet<char> = HashSet::new();
        for text in texts {
            if let Ok(plain) = text.to_str() {
                taken.extend(plain.chars());
                continue;
            }
            for character in text.try_iter()? {
                let character: Bound<'py, PyAny> = character?;
                match character.extract::<char>() {
                    Ok(plain) => {
                        taken.insert(plain);
                    }
                    Err(_) => {
                        surrogates.insert(ordinal(&character)?);
                    }
                }
            }
        }
        if surrogates.is_empty() {
            return Ok(Self { py, tables: None });
        }
        for name in reserved()? {
            taken.extend(name.chars());
        }
        let send: Bound<'py, PyDict> = PyDict::new(py);
        let back: Bound<'py, PyDict> = PyDict::new(py);
        let carriers = PRIVATE_USE
            .filter_map(char::from_u32)
            .filter(|candidate| !taken.contains(candidate));
        for (surrogate, carrier) in surrogates.into_iter().zip(carriers) {
            send.set_item(surrogate, carrier)?;
            back.set_item(u32::from(carrier), surrogate_text(py, surrogate)?)?;
        }
        Ok(Self {
            py,
            tables: Some((send, back)),
        })
    }

    /// A carrier for one text with nothing reserved.
    pub fn single(text: &Bound<'py, PyString>) -> PyResult<Self> {
        Self::new(text.py(), std::slice::from_ref(text), || Ok(Vec::new()))
    }

    /// The text with surrogates replaced by their carriers.
    pub fn send(&self, text: &Bound<'py, PyString>) -> PyResult<String> {
        match &self.tables {
            None => Ok(text.to_str()?.to_owned()),
            Some((send, _)) => text.call_method1("translate", (send,))?.extract(),
        }
    }

    /// The texts with surrogates replaced by their carriers.
    pub fn send_all(&self, texts: &[Bound<'py, PyString>]) -> PyResult<Vec<String>> {
        texts.iter().map(|text| self.send(text)).collect()
    }

    /// The text with carriers restored to their surrogates.
    pub fn back(&self, text: &str) -> PyResult<Bound<'py, PyString>> {
        let restored: Bound<'py, PyString> = PyString::new(self.py, text);
        match &self.tables {
            None => Ok(restored),
            Some((_, back)) => Ok(restored
                .call_method1("translate", (back,))?
                .downcast_into()?),
        }
    }

    pub fn py(&self) -> Python<'py> {
        self.py
    }

    /// The `PlannerInputError` instance for a selector failure, with carriers restored.
    pub fn exception(&self, error: SelectorError) -> PyResult<Bound<'py, PyAny>> {
        let keywords: Bound<'py, PyDict> = PyDict::new(self.py);
        keywords.set_item("code", error.code)?;
        let help: Option<Bound<'py, PyString>> = error
            .help
            .as_deref()
            .map(|help| self.back(help))
            .transpose()?;
        keywords.set_item("help", help)?;
        error_class(self.py, ErrorId::PlannerInput)?
            .call((self.back(&error.message)?,), Some(&keywords))
    }

    /// The `PlannerInputError` to raise for a selector failure, with carriers restored.
    pub fn error(&self, error: SelectorError) -> PyErr {
        match self.exception(error) {
            Ok(instance) => PyErr::from_value(instance),
            Err(failure) => failure,
        }
    }
}
