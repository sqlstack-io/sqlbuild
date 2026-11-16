//! The macro call memo class: one compile's recorded calls and its optional native store.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::Python;
use pyo3::{PyResult, pyclass, pymethods};
use sqlbuild_cache::store::models::NativeStore;
use std::path::PathBuf;
use std::sync::Mutex;

use sqlbuild_python_base::cache::main::open_store::open_store;
use sqlbuild_python_base::cache::main::save_store::save_store;

use crate::bindings::_helpers::render::macro_call_classes::CallClassState;

/// The native store kind holding recorded macro calls.
const MACRO_CALL_STORE_KIND: &str = "macro-calls";

/// The macro call results one compile records and replays, keyed by call class and call text.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug, Default)]
pub(crate) struct MacroCallMemo {
    pub(crate) inner: Mutex<sqlbuild_render::macro_calls::models::MacroCallMemo>,
    pub(crate) classes: Mutex<CallClassState>,
    /// `len(sys.modules)` when the store was last trusted, or `None` once it is not.
    pub(crate) observed_modules: Mutex<Option<usize>>,
}

pub(crate) fn poisoned<T>(_: T) -> pyo3::PyErr {
    PyValueError::new_err("macro call memo is poisoned")
}

#[pymethods]
impl MacroCallMemo {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Forget call tokens after attaching a store; `store_tokens` when its keys are computed.
    fn reset_call_classes(&self, store_tokens: bool) -> PyResult<()> {
        self.classes
            .lock()
            .map_err(poisoned)?
            .reset_tokens(store_tokens);
        Ok(())
    }

    /// Note how many modules were loaded when the store was trusted, or `None` once it is not.
    fn observe_modules(&self, count: Option<usize>) -> PyResult<()> {
        *self.observed_modules.lock().map_err(poisoned)? = count;
        Ok(())
    }

    /// Hits, misses and recorded results so far.
    fn stats(&self) -> PyResult<(usize, usize, usize)> {
        Ok(self.inner.lock().map_err(poisoned)?.stats())
    }

    /// Open the store at `path` for `environment`; returns its saved metadata and entry count.
    fn attach_store(
        &self,
        py: Python<'_>,
        path: PathBuf,
        environment: &str,
    ) -> PyResult<(Vec<u8>, usize)> {
        if sqlbuild_python_base::compile_context::main::delta::injection::injection().is_some() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "a delta compile leaves the macro call store to full compiles",
            ));
        }
        let store: NativeStore = open_store(py, &path, MACRO_CALL_STORE_KIND, environment)?;
        let opened = (store.metadata().to_vec(), store.loaded_entries());
        self.inner.lock().map_err(poisoned)?.attach_store(store);
        Ok(opened)
    }

    /// Drop the attached store's loaded entries after its metadata failed validation.
    fn discard_store(&self) -> PyResult<()> {
        if let Some(store) = self.inner.lock().map_err(poisoned)?.store_mut() {
            store.discard();
        }
        Ok(())
    }

    /// Whether this compile may use the call store; a delta compile leaves it to full compiles.
    fn store_allowed(&self) -> bool {
        sqlbuild_python_base::compile_context::main::delta::injection::injection().is_none()
    }

    /// Whether this compile recorded calls the attached store does not hold yet.
    fn store_needs_save(&self) -> PyResult<bool> {
        Ok(self
            .inner
            .lock()
            .map_err(poisoned)?
            .store_mut()
            .is_some_and(|store| store.needs_save()))
    }

    /// Calls found in the store and calls recorded into it.
    fn store_stats(&self) -> PyResult<(usize, usize)> {
        Ok(self.inner.lock().map_err(poisoned)?.store_stats())
    }

    /// Atomically save the store when this compile recorded into it; returns entries written.
    fn save_store(
        &self,
        py: Python<'_>,
        path: PathBuf,
        metadata: &[u8],
    ) -> PyResult<Option<usize>> {
        let mut memo = self.inner.lock().map_err(poisoned)?;
        let Some(store) = memo.store_mut() else {
            return Ok(None);
        };
        save_store(py, store, &path, metadata)
    }
}
