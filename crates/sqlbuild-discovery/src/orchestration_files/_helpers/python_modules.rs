//! A calendar or sensor module checked against its folder, without running it.

use sqlbuild_pysource::orchestration_modules::main::orchestration_module_declarations::orchestration_module_declarations;
use sqlbuild_pysource::orchestration_modules::models::{
    LiteralArgument, ModuleDeclarations, PythonDeclaration, PythonDeclarationKind, SensorForm,
    SourcePosition,
};

use crate::constants::PYTHON_FILE_SUFFIX;
use crate::orchestration_files::constants::{
    CALENDAR_KIND, PRIVATE_PREFIX, ROLE_FOLDER_DEPTH, SENSOR_DECORATOR_HELP, SENSOR_KIND,
    SENSOR_NAME_HELP, TOML_JOBS_HELP,
};
use crate::orchestration_files::errors::FileError;
use crate::orchestration_files::models::{FilePosition, NamedModule, Positioned};

fn file_position(position: SourcePosition) -> FilePosition {
    FilePosition {
        line: position.line,
        column: position.column,
    }
}

fn error(position: SourcePosition, message: String, help: Option<String>) -> FileError {
    FileError {
        message,
        position: file_position(position),
        help,
    }
}

fn label(kind: PythonDeclarationKind) -> (&'static str, &'static str) {
    match kind {
        PythonDeclarationKind::Calendar => CALENDAR_KIND,
        PythonDeclarationKind::Sensor => SENSOR_KIND,
    }
}

/// A literal string argument, or an error naming `what` and how to fix it.
fn literal(argument: LiteralArgument, what: &str) -> Result<Positioned<String>, FileError> {
    match argument.value {
        Some(value) if !value.trim().is_empty() => Ok(Positioned {
            value,
            position: file_position(argument.position),
        }),
        Some(_) => Err(error(
            argument.position,
            format!("{what} must not be blank"),
            None,
        )),
        None => Err(error(
            argument.position,
            format!("{what} must be a string literal"),
            Some(format!(
                "write the {what} as a quoted string, such as \"orders_job\", so the compiler can check it without running the module"
            )),
        )),
    }
}

/// A sensor declares itself only as `@sensor(...)` with keyword arguments above a function.
fn sensor_form(declaration: &PythonDeclaration) -> Result<(), FileError> {
    if let Some(position) = declaration.positional {
        return Err(error(
            position,
            "sensor takes keyword arguments only".to_owned(),
            Some(SENSOR_NAME_HELP.to_owned()),
        ));
    }
    match declaration.sensor_form {
        Some(SensorForm::Call) => Err(error(
            declaration.position,
            "sensor(...) must decorate a function".to_owned(),
            Some(SENSOR_DECORATOR_HELP.to_owned()),
        )),
        Some(SensorForm::BareDecorator) => Err(error(
            declaration.position,
            "@sensor needs its arguments".to_owned(),
            Some(SENSOR_DECORATOR_HELP.to_owned()),
        )),
        Some(SensorForm::Decorator { .. }) | None => Ok(()),
    }
}

/// The one declaration a public module at `path` in a `kind` folder makes; `None` for a private support module.
pub(crate) fn module_declaration(
    text: &str,
    path: &str,
    kind: PythonDeclarationKind,
    python_version: (u8, u8),
) -> Result<Option<NamedModule>, FileError> {
    let (folders, file_name) = path.rsplit_once('/').unwrap_or(("", path));
    let stem: &str = file_name
        .strip_suffix(PYTHON_FILE_SUFFIX)
        .unwrap_or(file_name);
    let private_folder: Option<&str> = folders
        .split('/')
        .skip(ROLE_FOLDER_DEPTH)
        .find(|folder| folder.starts_with(PRIVATE_PREFIX));
    let start = SourcePosition { line: 1, column: 1 };
    let Some(ModuleDeclarations {
        declarations,
        removed_imports,
    }) = orchestration_module_declarations(text, python_version)
    else {
        return Err(error(
            start,
            format!(
                "is not valid Python {}.{}",
                python_version.0, python_version.1
            ),
            None,
        ));
    };
    if let Some((name, position)) = removed_imports.into_iter().next() {
        return Err(error(
            position,
            format!(
                "`{name}` was removed from sqlbuild.orchestration; jobs, job groups and schedules are declared in TOML"
            ),
            Some(TOML_JOBS_HELP.to_owned()),
        ));
    }
    let (noun, suffix) = label(kind);
    if let Some(other) = declarations
        .iter()
        .find(|declaration| declaration.kind != kind)
    {
        let (other_noun, _) = label(other.kind);
        return Err(error(
            other.position,
            format!("declares a {other_noun}, which belongs in orchestration/{other_noun}s/"),
            None,
        ));
    }
    let private: bool = stem.starts_with(PRIVATE_PREFIX) || private_folder.is_some();
    let mut declarations = declarations.into_iter();
    let first: Option<PythonDeclaration> = declarations.next();
    if let Some(second) = declarations.next() {
        return Err(error(
            second.position,
            format!("declares a second {noun}; each module declares exactly one"),
            None,
        ));
    }
    let Some(declaration) = first else {
        if private {
            return Ok(None);
        }
        return Err(error(
            start,
            format!("declares no {noun}"),
            Some(format!(
                "declare one {noun}, or start the file name with `_` to make it a private support module"
            )),
        ));
    };
    if let Some(folder) = private_folder {
        return Err(error(
            declaration.position,
            format!(
                "declares a {noun} inside the private folder {folder}/; declarations can't live in a private folder"
            ),
            Some(format!(
                "move the module out of {folder}/ into its domain folder, or move the {noun} into a module there"
            )),
        ));
    }
    if private {
        return Err(error(
            declaration.position,
            format!("private support module declares a {noun}"),
            Some("remove the leading `_` from the file name".to_owned()),
        ));
    }
    sensor_form(&declaration)?;
    let name: Positioned<String> = match declaration.name {
        Some(name) => literal(name, &format!("{noun} name"))?,
        None => {
            return Err(error(
                declaration.position,
                format!("{noun} needs a name"),
                Some(format!("pass name=\"<name>{suffix}\"")),
            ));
        }
    };
    let job_references = declaration
        .job_references
        .into_iter()
        .map(|(call, value)| Ok((call, literal(value, &format!("sensor {call} job"))?)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(NamedModule {
        name: name.value,
        path: String::new(),
        position: name.position,
        job_references,
        targets: declaration.targets,
        condition: declaration.condition,
    }))
}
