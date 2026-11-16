//! Public traits and type aliases of compiled project.

use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;

/// One discovered source: its file's path, the discovered file and the entry.
pub type SourceRow = (String, Py<PyAny>, Py<PyAny>);

/// `(default contract, whether expressions are syntax-checked, source kind)`.
pub type SourceSettings = (Option<String>, bool, Py<PyAny>);

/// A test case's discovered case, name and parameter values in authored order.
pub type TestCaseRow = (Py<PyAny>, String, Vec<(String, Py<PyAny>)>);

/// The attached SQL test inputs, then scenario inputs.
pub type AttachedSqlTests = (Vec<Py<PyAny>>, Vec<Py<PyAny>>);

/// A loader's description, relative path and whether that path is a YAML file.
pub type LoaderRow = (Option<String>, String, bool);

/// The orphaned audit factory warnings, then the missing descriptions, then the cursor errors.
pub type ResourceDiagnostics<'py> = (
    Vec<Bound<'py, PyAny>>,
    Vec<Bound<'py, PyAny>>,
    Vec<Bound<'py, PyAny>>,
);

/// The tuple Python sends for a `FunctionSettings`, in field order.
pub type FunctionSettingsTuple = (
    String,
    bool,
    (Option<String>, Option<String>),
    Option<(Option<String>, Option<String>)>,
    bool,
    Py<PyAny>,
);

/// The tuple Python sends for a `TestBlockRow`, in field order.
pub type TestBlockTuple = (
    Py<PyAny>,
    Option<String>,
    String,
    String,
    Vec<String>,
    Vec<TestCaseRow>,
    Option<(usize, usize)>,
);
