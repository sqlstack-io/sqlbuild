//! `sqb playground`, `sqb skills` and generated-skill maintenance as `_native` functions.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pyo3::exceptions::PyUnicodeDecodeError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::workspace::errors::WorkspaceError;
use sqlbuild_config::workspace::main::create_playground::create_playground;
use sqlbuild_config::workspace::main::install_skills::install_skills;
use sqlbuild_config::workspace::main::maintain_skills::maintain_skills;
use sqlbuild_config::workspace::main::packaged_skill_files::packaged_skill_files;
use sqlbuild_config::workspace::main::playground_project_dir::playground_project_dir;
use sqlbuild_config::workspace::main::playground_summary::playground_summary;
use sqlbuild_config::workspace::models::{PlaygroundRequest, SkillInstallRequest};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_host::entries::models::FacadeHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::recall::console::write_text;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

const SKILLS_DIRECTORY: &str = ".agents/skills/sqlbuild";
const TEMPLATES_DIRECTORY: &str = "playground_templates";
const SKILLS_OPTIONS: (&[&str], &[&str]) =
    (&["project_dir"], &["global_install", "targets", "force"]);
const UPDATE_OPTIONS: (&[&str], &[&str]) = (
    &["project_dir"],
    &["global_install", "requested_targets", "force", "home_dir"],
);

/// The Python exception the workspace failure is raised as.
fn raised(py: Python<'_>, error: WorkspaceError) -> PyErr {
    match error {
        WorkspaceError::User {
            code,
            message,
            help,
        } => {
            let created = || -> PyResult<PyErr> {
                let keywords: Bound<'_, PyDict> = PyDict::new(py);
                keywords.set_item("code", code)?;
                keywords.set_item("help", help)?;
                let instance: Bound<'_, PyAny> =
                    error_class(py, ErrorId::CliUser)?.call((message,), Some(&keywords))?;
                Ok(PyErr::from_value(instance))
            };
            created().unwrap_or_else(|failure| failure)
        }
        WorkspaceError::Decode { bytes, error, .. } => {
            match PyUnicodeDecodeError::new_utf8(py, &bytes, error) {
                Ok(instance) => PyErr::from_value(instance.into_any()),
                Err(failure) => failure,
            }
        }
        WorkspaceError::Io(error) => PyErr::from(error),
    }
}

/// The installed `sqlbuild` package directory.
fn package_dir(py: Python<'_>) -> PyResult<PathBuf> {
    let file: PathBuf = host::entry(py, FacadeHost::PackageFile)?.extract()?;
    Ok(file.parent().map(Path::to_path_buf).unwrap_or_default())
}

fn home_dir(py: Python<'_>) -> PyResult<PathBuf> {
    imported(py, "pathlib", "Path")?
        .call_method0("home")?
        .extract()
}

fn current_dir(py: Python<'_>) -> PyResult<PathBuf> {
    imported(py, "pathlib", "Path")?
        .call_method0("cwd")?
        .extract()
}

fn install(py: Python<'_>, request: &SkillInstallRequest) -> PyResult<Vec<PathBuf>> {
    install_skills(request).map_err(|error| raised(py, error))
}

fn skill_request(py: Python<'_>, project_dir: PathBuf) -> PyResult<SkillInstallRequest> {
    Ok(SkillInstallRequest {
        project_dir,
        skills_root: package_dir(py)?.join(SKILLS_DIRECTORY),
        home_dir: home_dir(py)?,
        global_install: false,
        requested_targets: Vec::new(),
        force: false,
    })
}

/// Write packaged SQLBuild skill content to configured agent locations.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn update_sqlbuild_skills<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "update_sqlbuild_skills", options, UPDATE_OPTIONS)?;
    let mut request: SkillInstallRequest =
        skill_request(py, arguments.value("project_dir")?.extract()?)?;
    request.global_install = arguments.value_or_false("global_install")?.is_truthy()?;
    request.force = arguments.value_or_false("force")?.is_truthy()?;
    if arguments.passed("requested_targets")? {
        request.requested_targets = arguments.value("requested_targets")?.extract()?;
    }
    let home: Bound<'py, PyAny> = arguments.value("home_dir")?;
    if !home.is_none() {
        request.home_dir = home.extract()?;
    }
    let written: Vec<PathBuf> = install(py, &request)?;
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("written_paths", PyTuple::new(py, written)?)?;
    new_record(py, RecordId::SkillUpdateResult, (), Some(&keywords))
}

/// Check or refresh generated project skills without changing command success.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn maintain_sqlbuild_skills(
    py: Python<'_>,
    project_dir: PathBuf,
) -> PyResult<Bound<'_, PyAny>> {
    let message: String = maintain_skills(&project_dir, &package_dir(py)?.join(SKILLS_DIRECTORY))
        .map_err(|error| raised(py, error))?;
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("message", message)?;
    new_record(py, RecordId::SkillMaintenanceResult, (), Some(&keywords))
}

/// Return every packaged skill file keyed by skill-relative path, with generated markers.
#[pyfunction]
fn load_packaged_skill_files(py: Python<'_>) -> PyResult<BTreeMap<String, String>> {
    packaged_skill_files(&package_dir(py)?.join(SKILLS_DIRECTORY))
        .map_err(|error| raised(py, error))
}

/// `sqb skills`: install or update SQLBuild skill files.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_skills_command(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<i32> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "run_skills_command", options, SKILLS_OPTIONS)?;
    let project_dir: Bound<'_, PyAny> = arguments.value("project_dir")?;
    let base: PathBuf = if project_dir.is_none() {
        current_dir(py)?
    } else {
        project_dir.extract()?
    };
    let mut request: SkillInstallRequest = skill_request(py, base)?;
    request.global_install = arguments.value_or_false("global_install")?.is_truthy()?;
    request.force = arguments.value_or_false("force")?.is_truthy()?;
    if arguments.passed("targets")? {
        request.requested_targets = arguments.value("targets")?.extract()?;
    }
    let written: Vec<PathBuf> = install(py, &request)?;
    let mut text: String = "Updated SQLBuild skill files:\n".to_owned();
    for path in written {
        text.push_str(&format!("  {}\n", path.display()));
    }
    write_text(py, "stdout", &text)?;
    Ok(0)
}

/// `sqb playground`: create a self-contained playground project.
#[pyfunction]
pub(crate) fn run_playground_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let project_dir: Bound<'_, PyAny> = native_field(request, "project_dir")?;
    let base: PathBuf = if project_dir.is_none() {
        current_dir(py)?
    } else {
        project_dir.extract()?
    };
    let display_path: String = native_field(request, "target_path")?.str()?.extract()?;
    let target_path: PathBuf = PathBuf::from(&display_path);
    let target_dir: PathBuf = if target_path.is_absolute() {
        target_path
    } else {
        base.join(target_path)
    };
    let template: String = native_field(request, "template")?.str()?.extract()?;
    create_playground(&PlaygroundRequest {
        templates_root: package_dir(py)?.join(TEMPLATES_DIRECTORY),
        target_dir: target_dir.clone(),
        template: template.clone(),
    })
    .map_err(|error| raised(py, error))?;
    install(
        py,
        &skill_request(py, playground_project_dir(&target_dir, &template))?,
    )?;
    let use_color: bool = stdout_supports_color(py)?;
    write_text(
        py,
        "stdout",
        &playground_summary(&display_path, &template, CliStyle { use_color }),
    )?;
    Ok(0)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(update_sqlbuild_skills, module)?)?;
    module.add_function(wrap_pyfunction!(maintain_sqlbuild_skills, module)?)?;
    module.add_function(wrap_pyfunction!(load_packaged_skill_files, module)?)?;
    module.add_function(wrap_pyfunction!(run_skills_command, module)?)?;
    module.add_function(wrap_pyfunction!(run_playground_command, module)?)
}
