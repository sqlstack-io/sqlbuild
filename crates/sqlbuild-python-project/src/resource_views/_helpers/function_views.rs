//! Functions attached by the native loop, held by the compiled project, and their views.

use crate::resource_views::models::NativeCompiledFunction;
use crate::resource_views::models::NativeFunctionInput;
use crate::resource_views::types::AttachedFunction;
use crate::resource_views::types::FunctionRecord;
use pyo3::exceptions::{PyIndexError, PyRuntimeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{FromPyObject, pymethods};
use sqlbuild_python_base::positional_record;

use sqlbuild_project::types::{NamedTypeRow, ObjectKeyRow};

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::main::compiled_models::defaulted_destination::defaulted_destination;
use crate::compiled_views::main::compiled_models::object_key::object_key;
use crate::compiled_views::main::compiled_models::relation_location::relation_location;
use crate::compiled_views::main::model_inputs::reference_tuple::reference_tuple;
use crate::compiled_views::main::model_inputs::view_classes::view_classes;
use crate::compiled_views::models::InputViewClasses;
use sqlbuild_project::models::DestinationFact;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const SQL_LANGUAGE: &str = "sql";
const PYTHON_LANGUAGE: &str = "python";
const FUNCTION_FILE_ATTRIBUTE: &str = "relative_path";
const FILE_PATH_ATTRIBUTE: &str = "file_path";
pub(crate) const NOT_ASSEMBLED: &str = "the function was not assembled";
const FUNCTION_CLASSES: &str = "functions";

/// The Python classes a function view projects its argument and return column records with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct FunctionClasses<'py> {
    function_argument: Bound<'py, PyAny>,
    function_return_column: Bound<'py, PyAny>,
    function_language: Bound<'py, PyAny>,
}

positional_record! {
    into;
    /// `(node type, name, tags, references as (kind, name, package))`.
    pub(crate) struct FunctionAssemblyRow {
        node_type: &'static str,
        name: String,
        tags: Vec<String>,
        references: Vec<(String, String, Option<String>)>,
    }
}

/// `rows` as `class` records, built once into `cache`.
fn named_types(
    rows: &[NamedTypeRow],
    class: &Bound<'_, PyAny>,
    cache: &PyOnceLock<Py<PyTuple>>,
) -> PyResult<Py<PyTuple>> {
    let py: Python<'_> = class.py();
    cache
        .get_or_try_init(py, || {
            let mut items: Vec<Bound<'_, PyAny>> = Vec::with_capacity(rows.len());
            for (name, type_text) in rows {
                let fields: Bound<'_, PyDict> = PyDict::new(py);
                fields.set_item("name", name)?;
                fields.set_item("type", type_text)?;
                items.push(class.call((), Some(&fields))?);
            }
            Ok::<_, pyo3::PyErr>(PyTuple::new(py, items)?.unbind())
        })
        .map(|items| items.clone_ref(py))
}

/// The function's `FunctionLanguage` member.
pub(crate) fn language<'py>(
    function: &FunctionRecord,
    classes: &FunctionClasses<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let value: &str = if function.attached.python.is_some() {
        PYTHON_LANGUAGE
    } else {
        SQL_LANGUAGE
    };
    classes.function_language.call1((value,))
}

pub(crate) fn read_function<'py, T>(
    owner: &Py<NativeCompiledProject>,
    index: usize,
    py: Python<'py>,
    read: impl FnOnce(&FunctionRecord, Python<'py>) -> PyResult<T>,
) -> PyResult<T> {
    let owner = owner.bind(py).borrow();
    let function: &FunctionRecord = owner
        .functions()
        .get(index)
        .ok_or_else(|| PyIndexError::new_err(index))?;
    read(function, py)
}

fn function_classes<'py>(
    owner: &Py<NativeCompiledProject>,
    py: Python<'py>,
) -> PyResult<FunctionClasses<'py>> {
    owner
        .bind(py)
        .borrow()
        .input_classes(py)?
        .get_item(FUNCTION_CLASSES)?
        .extract()
}

/// The attributes a function's input and compiled views share.
macro_rules! function_getters {
    ($view:ty, { $($extra:tt)* }) => {
        #[pymethods]
        impl $view {
            $($extra)*

            #[getter]
            pub(crate) fn name(&self, py: Python<'_>) -> PyResult<String> {
                read_function(&self.owner, self.index, py, |function, _| {
                    Ok(function.attached.name.clone())
                })
            }

            #[getter]
            pub(crate) fn arguments(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
                let classes: FunctionClasses<'_> = function_classes(&self.owner, py)?;
                read_function(&self.owner, self.index, py, |function, _| {
                    named_types(
                        &function.attached.arguments,
                        &classes.function_argument,
                        &function.host.arguments,
                    )
                })
            }

            #[getter]
            pub(crate) fn returns(&self, py: Python<'_>) -> PyResult<String> {
                read_function(&self.owner, self.index, py, |function, _| {
                    Ok(function.attached.returns.clone())
                })
            }

            #[getter]
            pub(crate) fn body_sql(&self, py: Python<'_>) -> PyResult<String> {
                read_function(&self.owner, self.index, py, |function, _| {
                    Ok(function.attached.body.clone())
                })
            }

            #[getter]
            pub(crate) fn return_columns(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
                let classes: FunctionClasses<'_> = function_classes(&self.owner, py)?;
                read_function(&self.owner, self.index, py, |function, _| {
                    named_types(
                        &function.attached.return_columns,
                        &classes.function_return_column,
                        &function.host.return_columns,
                    )
                })
            }

            #[getter]
            pub(crate) fn references(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
                let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                read_function(&self.owner, self.index, py, |function, _| {
                    reference_tuple(
                        &classes,
                        &function.attached.references,
                        &function.host.references,
                    )
                })
            }

            #[getter]
            pub(crate) fn language<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                let classes: FunctionClasses<'py> = function_classes(&self.owner, py)?;
                read_function(&self.owner, self.index, py, |function, _| {
                    language(function, &classes)
                })
            }

            #[getter]
            pub(crate) fn runtime_version(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_function(&self.owner, self.index, py, |function, _| {
                    Ok(function
                        .attached
                        .python
                        .as_ref()
                        .and_then(|python| python.runtime_version.clone()))
                })
            }

            #[getter]
            pub(crate) fn entry_point(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_function(&self.owner, self.index, py, |function, _| {
                    Ok(function
                        .attached
                        .python
                        .as_ref()
                        .and_then(|python| python.entry_point.clone()))
                })
            }

            #[getter]
            pub(crate) fn packages<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
                read_function(&self.owner, self.index, py, |function, py| {
                    let packages: &[String] = function
                        .attached
                        .python
                        .as_ref()
                        .map_or(&[], |python| python.packages.as_slice());
                    PyTuple::new(py, packages)
                })
            }

            #[getter]
            pub(crate) fn tags<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
                read_function(&self.owner, self.index, py, |function, py| {
                    PyTuple::new(py, &function.attached.tags)
                })
            }

            #[getter]
            pub(crate) fn description(&self, py: Python<'_>) -> PyResult<Option<String>> {
                read_function(&self.owner, self.index, py, |function, _| {
                    Ok(function.attached.description.clone())
                })
            }
        }
    };
}

function_getters!(NativeFunctionInput, {
    #[getter]
    pub(crate) fn function_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_function(&self.owner, self.index, py, |function, py| {
            Ok(function.attached.host.function_file.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn database(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function.attached.destination.database.clone())
        })
    }

    #[getter]
    pub(crate) fn schema(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function.attached.destination.schema.clone())
        })
    }

    #[getter]
    pub(crate) fn logical_database(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function.attached.destination.logical_database.clone())
        })
    }

    #[getter]
    pub(crate) fn logical_schema(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function.attached.destination.logical_schema.clone())
        })
    }

    #[getter]
    pub(crate) fn fingerprint_database(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function.attached.fingerprint_destination.database.clone())
        })
    }

    #[getter]
    pub(crate) fn fingerprint_schema(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function.attached.fingerprint_destination.schema.clone())
        })
    }

    #[getter]
    pub(crate) fn fingerprint_logical_database(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function
                .attached
                .fingerprint_destination
                .logical_database
                .clone())
        })
    }

    #[getter]
    pub(crate) fn fingerprint_logical_schema(&self, py: Python<'_>) -> PyResult<Option<String>> {
        read_function(&self.owner, self.index, py, |function, _| {
            Ok(function
                .attached
                .fingerprint_destination
                .logical_schema
                .clone())
        })
    }

    #[getter]
    pub(crate) fn declaration_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        read_function(&self.owner, self.index, py, |function, py| {
            Ok(function.attached.host.usages.clone_ref(py))
        })
    }

    /// `(node type, name, tags, references)`, what project assembly resolves dependencies from.
    #[getter]
    pub(crate) fn assembly_row(&self, py: Python<'_>) -> PyResult<FunctionAssemblyRow> {
        read_function(&self.owner, self.index, py, |function, _| {
            let attached: &AttachedFunction = &function.attached;
            Ok(FunctionAssemblyRow {
                node_type: attached.node_type(),
                name: attached.name.clone(),
                tags: attached.tags.clone(),
                references: attached
                    .references
                    .iter()
                    .map(
                        |ReferenceRow {
                             kind,
                             name,
                             package,
                             ..
                         }| (kind.clone(), name.clone(), package.clone()),
                    )
                    .collect(),
            })
        })
    }
});

function_getters!(NativeCompiledFunction, {
    /// The function's planner-facing `CompiledObjectKey`.
    #[getter]
    pub(crate) fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        read_function(&self.owner, self.index, py, |function, py| {
            function
                .host
                .key
                .get_or_try_init(py, || {
                    object_key(
                        &classes.compiled,
                        function.attached.node_type(),
                        &function.attached.name,
                    )
                })
                .map(|key| key.clone_ref(py))
        })
    }

    /// The planner-facing keys of the resources the function reads.
    #[getter]
    pub(crate) fn deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        read_function(&self.owner, self.index, py, |function, py| {
            let deps: &[ObjectKeyRow] = function
                .deps
                .as_deref()
                .ok_or_else(|| PyRuntimeError::new_err(NOT_ASSEMBLED))?;
            let mut keys: Vec<Py<PyAny>> = Vec::with_capacity(deps.len());
            for (kind, name) in deps {
                keys.push(object_key(&classes.compiled, kind, name)?);
            }
            PyTuple::new(py, keys)
        })
    }

    #[getter]
    pub(crate) fn relative_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_function(&self.owner, self.index, py, |function, py| {
            Ok(native_field(
                function.attached.host.function_file.bind(py),
                FUNCTION_FILE_ATTRIBUTE,
            )?
            .unbind())
        })
    }

    #[getter]
    pub(crate) fn source_file_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        read_function(&self.owner, self.index, py, |function, py| {
            Ok(native_field(
                function.attached.host.function_file.bind(py),
                FILE_PATH_ATTRIBUTE,
            )?
            .unbind())
        })
    }

    /// The function's planner-facing `CompiledRelationLocation`.
    #[getter]
    pub(crate) fn destination(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.destination
            .get_or_try_init(py, || {
                relation_location(&classes.compiled, &self.current_destinations(py)?.0)
            })
            .map(|destination| destination.clone_ref(py))
    }

    /// The destination the function's fingerprint reads.
    #[getter]
    pub(crate) fn fingerprint_destination(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.fingerprint_destination
            .get_or_try_init(py, || {
                relation_location(&classes.compiled, &self.current_destinations(py)?.1)
            })
            .map(|destination| destination.clone_ref(py))
    }

    /// This function with target defaults; a non-inheriting Python function is name-qualified.
    pub fn with_target_defaults(
        &self,
        py: Python<'_>,
        defaults: (Option<String>, Option<String>),
        render: Bound<'_, PyAny>,
        python_functions_inherit_default_namespace: bool,
    ) -> PyResult<Self> {
        let python: bool = read_function(&self.owner, self.index, py, |function, _| {
            Ok(function.attached.python.is_some())
        })?;
        let apply: bool = !python || python_functions_inherit_default_namespace;
        let (current, fingerprint) = self.current_destinations(py)?;
        let fingerprint: DestinationFact =
            defaulted_destination(&fingerprint, defaults.clone(), &render)?;
        let defaults: (Option<String>, Option<String>) =
            if apply { defaults } else { (None, None) };
        let mut defaulted: DestinationFact = defaulted_destination(&current, defaults, &render)?;
        if !apply && defaulted.qualified_name.is_none() {
            defaulted.qualified_name = Some(defaulted.name.clone());
        }
        Ok(Self::defaulted(
            self.owner.clone_ref(py),
            self.index,
            Some((defaulted, fingerprint)),
        ))
    }
});
