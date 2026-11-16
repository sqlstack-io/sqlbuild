//! Decorated Python nodes, audit factories, hooks and materializations from imported modules.

use crate::discovery::main::python_files::provider_by_name::provider_by_name;
use crate::discovery::main::python_files::provider_usages::provider_usages;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyList, PyString, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_core::constants::PYTHON_NODE_ROOT;
use sqlbuild_host::entries::models::UserCodeHost;

use crate::discovery::_helpers::python_files::imported_modules::{
    ProjectDir, definition, discovered, discovery_error, discovery_error_from, is_exception,
    module_functions, shim, tuple_of,
};
use crate::discovery::_helpers::python_files::python_hook_files::discover_python_hook_files;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
const NODE_ERROR: ErrorId = ErrorId::PythonNodeDiscovery;
const AUDIT_CASE: UserCodeHost = UserCodeHost::AuditCase;
const READ_FACTORY: UserCodeHost = UserCodeHost::ReadFactoryDefinition;
const READ_AUDIT_FACTORY: UserCodeHost = UserCodeHost::ReadAuditFactoryDefinition;
const READ_HOOK: UserCodeHost = UserCodeHost::ReadHookDefinition;
const NOT_A_NODE: &str = "that is not a SQLBuild task, asset, loader, or check";

/// One Python-node kind: its reader, record class, bucket and copied definition fields.
struct NodeKind {
    reader: UserCodeHost,
    record: RecordId,
    bucket: usize,
    /// The node kind and whether its identity hashes documentation fields.
    identity: (&'static str, bool),
    fields: &'static [&'static str],
}

/// Readers in precedence order; the first matching definition decides the kind.
const NODE_KINDS: [NodeKind; 4] = [
    NodeKind {
        reader: UserCodeHost::ReadLoaderDefinition,
        record: RecordId::DiscoveredLoaderFunction,
        bucket: 0,
        identity: ("LOADER", false),
        fields: &[
            "depends_on",
            "destination",
            "write_strategy",
            "cursor_column",
            "unique_key",
            "columns",
            "contract",
            "description",
        ],
    },
    NodeKind {
        reader: UserCodeHost::ReadTaskDefinition,
        record: RecordId::DiscoveredTaskFunction,
        bucket: 1,
        identity: ("TASK", true),
        fields: &[
            "depends_on",
            "tags",
            "group",
            "description",
            "meta",
            "retry",
        ],
    },
    NodeKind {
        reader: UserCodeHost::ReadAssetDefinition,
        record: RecordId::DiscoveredAssetFunction,
        bucket: 2,
        identity: ("ASSET", true),
        fields: &[
            "depends_on",
            "tags",
            "group",
            "description",
            "meta",
            "columns",
            "column_lineage",
            "retry",
        ],
    },
    NodeKind {
        reader: UserCodeHost::ReadCheckDefinition,
        record: RecordId::DiscoveredCheckFunction,
        bucket: 3,
        identity: ("CHECK", true),
        fields: &[
            "depends_on",
            "severity",
            "tags",
            "group",
            "description",
            "meta",
        ],
    },
];

/// Discovered loaders, tasks, assets, checks and audit factories, in discovery order.
pub(crate) struct Buckets<'py> {
    pub(crate) nodes: [Vec<Bound<'py, PyAny>>; 4],
    pub(crate) audit_factories: Vec<(String, String, Bound<'py, PyAny>)>,
}

/// The module and the file it was imported from.
struct ImportedModule<'py> {
    file_path: Bound<'py, PyAny>,
    module: Bound<'py, PyAny>,
}

impl<'py> Buckets<'py> {
    /// Append `function` to its kind's bucket; `false` when it carries no node definition.
    fn append_node(
        &mut self,
        (project, file_path): (&ProjectDir<'py>, &Bound<'py, PyAny>),
        function: &Bound<'py, PyAny>,
        providers: &Bound<'py, PyDict>,
    ) -> PyResult<bool> {
        let py: Python<'py> = project.py();
        for kind in &NODE_KINDS {
            let Some(read) = definition(function, kind.reader)? else {
                continue;
            };
            let mut pairs: Vec<(&str, Bound<'py, PyAny>)> = vec![
                ("file_path", file_path.clone()),
                ("relative_path", project.relative(file_path)?),
                ("name", native_field(&read, "name")?),
                ("function", function.clone()),
            ];
            for field in kind.fields {
                pairs.push((field, native_field(&read, field)?));
            }
            pairs.push(("provider_usages", provider_usages(function, providers)?));
            let get = |name: &str| -> PyResult<Bound<'py, PyAny>> {
                match pairs.iter().find(|(field, _)| *field == name) {
                    Some((_, value)) => Ok(value.clone()),
                    None => Ok(py.None().into_bound(py)),
                }
            };
            let identity: Bound<'py, PyAny> = node_identity(py, kind.identity, &get)?;
            pairs.push(("identity", identity));
            self.nodes[kind.bucket].push(discovered(py, kind.record, &pairs)?);
            return Ok(true);
        }
        Ok(false)
    }

    /// Collect audit factories without routing them through Python-node discovery.
    fn append_audit_factories(
        &mut self,
        project: &ProjectDir<'py>,
        imported_module: &ImportedModule<'py>,
    ) -> PyResult<()> {
        let py: Python<'py> = project.py();
        for function in module_functions(&imported_module.module)? {
            let Some(read) = definition(&function, READ_AUDIT_FACTORY)? else {
                continue;
            };
            let name: String = native_field(&read, "name")?.extract()?;
            let relative: Bound<'py, PyAny> = project.relative(&imported_module.file_path)?;
            let relative_text: String = relative.str()?.extract()?;
            let kinds: Vec<&str> = node_definition_names(&function)?;
            if !kinds.is_empty() {
                return Err(discovery_error(
                    py,
                    NODE_ERROR,
                    format!(
                        "Audit factory '{name}' in {relative_text} cannot also be decorated as a Python-node kind ({})",
                        kinds.join(", ")
                    ),
                ));
            }
            if let Some((_, existing, _)) = self
                .audit_factories
                .iter()
                .find(|(listed, _, _)| *listed == name)
            {
                return Err(discovery_error(
                    py,
                    NODE_ERROR,
                    format!(
                        "Duplicate audit factory name '{name}' found in {existing} and {relative_text}"
                    ),
                ));
            }
            let cases: Vec<Bound<'py, PyAny>> = audit_cases(&function, (&name, &relative_text))?;
            let line: Bound<'py, PyAny> =
                shim(py, UserCodeHost::SourceLine)?.call1((&function,))?;
            let record: Bound<'py, PyAny> = discovered(
                py,
                RecordId::DiscoveredAuditFactory,
                &[
                    ("name", native_field(&read, "name")?),
                    ("function", function.clone()),
                    ("file_path", imported_module.file_path.clone()),
                    ("relative_path", relative),
                    ("line", line),
                    ("cases", tuple_of(py, cases)?),
                ],
            )?;
            self.audit_factories.push((name, relative_text, record));
        }
        Ok(())
    }

    fn append_factory_nodes(
        &mut self,
        (project, imported_module): (&ProjectDir<'py>, &ImportedModule<'py>),
        providers: &Bound<'py, PyDict>,
        direct: &HashMap<usize, Bound<'py, PyAny>>,
    ) -> PyResult<()> {
        let py: Python<'py> = project.py();
        let location: String = project.relative_to(&imported_module.file_path)?;
        for function in module_functions(&imported_module.module)? {
            let Some(read) = definition(&function, READ_FACTORY)? else {
                continue;
            };
            let name: String = native_field(&read, "name")?.extract()?;
            let result: Bound<'py, PyAny> = call_factory(&function, ("Factory", &name, &location))?;
            for (index, generated) in factory_functions(&result, (&name, &location))?
                .into_iter()
                .enumerate()
            {
                if direct
                    .get(&(generated.as_ptr() as usize))
                    .is_some_and(|known| known.is(&generated))
                {
                    continue;
                }
                if !self.append_node(
                    (project, &imported_module.file_path),
                    &generated,
                    providers,
                )? {
                    return Err(discovery_error(
                        py,
                        NODE_ERROR,
                        format!(
                            "Factory '{name}' in {location} returned item {index} {NOT_A_NODE}"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn discover_nodes(
        &mut self,
        project: &ProjectDir<'py>,
        providers: &Bound<'py, PyDict>,
    ) -> PyResult<()> {
        let mut modules: Vec<ImportedModule<'py>> = Vec::new();
        for file_path in project.module_files(PYTHON_NODE_ROOT, false)? {
            let module: Bound<'py, PyAny> =
                project.load(UserCodeHost::ImportPythonRootModule, &file_path)?;
            modules.push(ImportedModule { file_path, module });
        }
        let mut direct: HashMap<usize, Bound<'py, PyAny>> = HashMap::new();
        for imported_module in &modules {
            self.append_audit_factories(project, imported_module)?;
            for function in module_functions(&imported_module.module)? {
                if self.append_node((project, &imported_module.file_path), &function, providers)? {
                    direct.insert(function.as_ptr() as usize, function);
                }
            }
        }
        for imported_module in &modules {
            self.append_factory_nodes((project, imported_module), providers, &direct)?;
        }
        Ok(())
    }
}

/// The Python-node kinds an audit factory is also decorated as.
fn node_definition_names(function: &Bound<'_, PyAny>) -> PyResult<Vec<&'static str>> {
    let mut names: Vec<&'static str> = Vec::new();
    if definition(function, READ_FACTORY)?.is_some() {
        names.push("factory");
    }
    for (kind, name) in NODE_KINDS.iter().zip(["loader", "task", "asset", "check"]) {
        if definition(function, kind.reader)?.is_some() {
            names.push(name);
        }
    }
    Ok(names)
}

fn requires_arguments(function: &Bound<'_, PyAny>) -> PyResult<bool> {
    shim(function.py(), UserCodeHost::RequiresArguments)?
        .call1((function,))?
        .is_truthy()
}

/// Call a zero-argument discovery factory, wrapping its failure.
fn call_factory<'py>(
    factory: &Bound<'py, PyAny>,
    (label, name, location): (&str, &str, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = factory.py();
    if requires_arguments(factory)? {
        return Err(discovery_error(
            py,
            NODE_ERROR,
            format!("{label} '{name}' in {location} must not require arguments"),
        ));
    }
    factory.call0().map_err(|error| {
        if !is_exception(py, &error) {
            return error;
        }
        let message: String = format!(
            "{label} '{name}' in {location} failed during discovery: {}",
            error.value(py)
        );
        discovery_error_from(py, (NODE_ERROR, message), error)
    })
}

fn audit_cases<'py>(
    factory: &Bound<'py, PyAny>,
    (name, relative_path): (&str, &str),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = factory.py();
    let result: Bound<'py, PyAny> = call_factory(factory, ("Audit factory", name, relative_path))?;
    if !result.is_instance_of::<PyList>() && !result.is_instance_of::<PyTuple>() {
        return Err(discovery_error(
            py,
            NODE_ERROR,
            format!(
                "Audit factory '{name}' in {relative_path} must return a list or tuple of AuditCase instances"
            ),
        ));
    }
    let audit_case: Bound<'py, PyAny> = host_entry(py, AUDIT_CASE)?;
    let mut cases: Vec<Bound<'py, PyAny>> = Vec::new();
    for (index, item) in result.try_iter()?.enumerate() {
        let item: Bound<'py, PyAny> = item?;
        if !item.is_instance(&audit_case)? {
            return Err(discovery_error(
                py,
                NODE_ERROR,
                format!(
                    "Audit factory '{name}' in {relative_path} returned item {index} that is not an AuditCase"
                ),
            ));
        }
        cases.push(item);
    }
    Ok(cases)
}

/// The functions a factory returned, rejecting anything but node functions.
fn factory_functions<'py>(
    result: &Bound<'py, PyAny>,
    (name, location): (&str, &str),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = result.py();
    if result.is_callable() {
        return Ok(vec![result.clone()]);
    }
    let builtins: Bound<'py, PyAny> = py.import("builtins")?.into_any();
    let excluded: bool = result.is_instance_of::<PyString>()
        || result.is_instance_of::<PyBytes>()
        || result.is_instance_of::<PyDict>();
    let collection: bool = result.is_instance_of::<PyList>()
        || result.is_instance_of::<PyTuple>()
        || result.is_instance(&native_field(&builtins, "set")?)?;
    if excluded || !collection {
        return Err(discovery_error(
            py,
            NODE_ERROR,
            format!(
                "Factory '{name}' in {location} must return a SQLBuild node function or a list, tuple, or set of node functions"
            ),
        ));
    }
    let mut functions: Vec<Bound<'py, PyAny>> = Vec::new();
    for (index, item) in result.try_iter()?.enumerate() {
        let item: Bound<'py, PyAny> = item?;
        if !item.is_callable() {
            return Err(discovery_error(
                py,
                NODE_ERROR,
                format!("Factory '{name}' in {location} returned item {index} {NOT_A_NODE}"),
            ));
        }
        functions.push(item);
    }
    Ok(functions)
}

/// Decorated model lifecycle hook functions under hooks/python/.
#[pyfunction]
#[pyo3(signature = (*, project_dir, providers = None))]
pub(crate) fn discover_hook_functions<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project = ProjectDir { path: project_dir };
    let providers: Bound<'py, PyDict> = match providers {
        Some(providers) => provider_by_name(&providers)?,
        None => PyDict::new(py),
    };
    let hook_files: Bound<'py, PyAny> = discover_python_hook_files(&project.path.clone())
        .and_then(|value| value.into_bound_py_any(py))?;
    let mut hooks: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut seen: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    for hook_file in hook_files.try_iter()? {
        let hook_file: Bound<'py, PyDict> = hook_file?.cast_into()?;
        let file_path: Bound<'py, PyAny> = hook_file.as_any().get_item("file_path")?;
        let module: Bound<'py, PyAny> =
            project.load(UserCodeHost::LoadPythonNodeModule, &file_path)?;
        for function in module_functions(&module)? {
            let Some(read) = definition(&function, READ_HOOK)? else {
                continue;
            };
            let name: String = native_field(&read, "name")?.extract()?;
            if let Some((_, existing)) = seen.iter().find(|(listed, _)| *listed == name) {
                return Err(discovery_error(
                    py,
                    NODE_ERROR,
                    format!(
                        "Duplicate hook name '{name}' found in {} and {}",
                        project.relative_to(existing)?,
                        project.relative_to(&file_path)?
                    ),
                ));
            }
            seen.push((name, file_path.clone()));
            let arguments: Bound<'py, PyDict> = hook_file.copy()?;
            for (field, value) in [
                ("name", native_field(&read, "name")?),
                ("function", function.clone()),
                ("description", native_field(&read, "description")?),
                ("reads", native_field(&read, "reads")?),
                ("provider_usages", provider_usages(&function, &providers)?),
            ] {
                arguments.set_item(field, value)?;
            }
            hooks.push(new_record(
                py,
                RecordId::DiscoveredHookFunction,
                (),
                Some(&arguments),
            )?);
        }
    }
    tuple_of(py, hooks)
}

fn optional_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value.clone());
    }
    native_field(value, "value")
}

/// The decorator configuration the identity of one node of `kind` hashes, read through `get`.
fn decorator_config<'py>(
    py: Python<'py>,
    (kind, documented): (&str, bool),
    get: &dyn Fn(&str) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let config: Bound<'py, PyDict> = PyDict::new(py);
    match kind {
        "LOADER" => {
            config.set_item("columns", get("columns")?)?;
            config.set_item("contract", get("contract")?)?;
            config.set_item("cursor_column", get("cursor_column")?)?;
            config.set_item("destination", get("destination")?)?;
            config.set_item("unique_key", get("unique_key")?)?;
            config.set_item("write_strategy", optional_value(&get("write_strategy")?)?)?;
        }
        "ASSET" => {
            config.set_item("columns", get("columns")?)?;
            config.set_item("column_lineage", get("column_lineage")?)?;
        }
        "CHECK" => {
            config.set_item("severity", get("severity")?.getattr("value")?)?;
        }
        _ => {}
    }
    if documented {
        for field in ["description", "group", "meta", "tags"] {
            config.set_item(field, get(field)?)?;
        }
    }
    Ok(config)
}

/// The identity of one discovered node of `(kind, documented)`, hashed once by the user-code host.
pub(crate) fn node_identity<'py>(
    py: Python<'py>,
    (kind, documented): (&str, bool),
    get: &dyn Fn(&str) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let kind_value: Bound<'py, PyAny> = host_entry(py, UserCodeHost::PythonNodeKind)?
        .getattr(kind)?
        .getattr("value")?;
    call_with(
        &host_entry(py, UserCodeHost::PythonNodeIdentity)?,
        &[
            ("kind", kind_value),
            ("name", get("name")?),
            ("function", get("function")?),
            ("file_path", get("file_path")?),
            (
                "decorator_config",
                decorator_config(py, (kind, documented), get)?.into_any(),
            ),
        ],
    )
}
