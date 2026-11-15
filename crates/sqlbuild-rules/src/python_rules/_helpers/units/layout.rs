//! What a unit folder holds, and nodes and checks declared outside their unit's files.

use std::collections::BTreeSet;

use sqlbuild_pysource::node_units::models::UnitFunction;

use crate::models::{Fault, PythonSource};
use crate::python_rules::_helpers::engine::faults::catalogue_fault;
use crate::python_rules::_helpers::engine::placement::package_anchor;
use crate::python_rules::_helpers::units::project::{
    Declared, declared, is_role_folder, top_functions, unit_fault, unit_file_name,
};
use crate::python_rules::constants::{
    NODE_DECLARED_IN_UNIT_CODE, NODE_UNIT_LAYOUT_CODE, PACKAGE_ROOT, UNIT_CHECKS_FILE,
    UNIT_HELPERS_FOLDER, UNIT_MAIN_FILE, UNIT_NODE_FILE, UNIT_ROLE_FILES,
};
use crate::python_rules::models::{UnitFile, UnitProject};

/// A unit without `main.py`, and modules or folders a unit may not hold.
pub(crate) fn unit_layout_faults(
    project: &UnitProject<'_>,
    sources: &[PythonSource],
) -> Vec<Fault> {
    let mut faults: Vec<Fault> = project
        .units
        .iter()
        .filter(|unit| {
            !project
                .paths
                .contains(format!("{PACKAGE_ROOT}/{unit}/{UNIT_MAIN_FILE}").as_str())
        })
        .filter_map(|unit| {
            catalogue_fault(
                NODE_UNIT_LAYOUT_CODE,
                &format!("{PACKAGE_ROOT}/{unit}/{UNIT_NODE_FILE}"),
                (1, 1),
                &format!("{PACKAGE_ROOT}/{unit}/ has no {UNIT_MAIN_FILE}"),
            )
        })
        .collect();
    let mut folders: BTreeSet<String> = BTreeSet::new();
    for path in &project.paths {
        let Some(rest) = path.strip_prefix(&format!("{PACKAGE_ROOT}/")) else {
            continue;
        };
        let parts: Vec<&str> = rest.split('/').collect();
        let Some(depth) = (1..parts.len())
            .rev()
            .find(|depth| project.units.contains(&parts[..*depth].join("/")))
        else {
            continue;
        };
        let unit_path: String = format!("{PACKAGE_ROOT}/{}", parts[..depth].join("/"));
        let entry: &str = parts[depth];
        if parts.len() == depth + 1 {
            let allowed: bool = [UNIT_NODE_FILE, UNIT_MAIN_FILE].contains(&entry)
                || UNIT_ROLE_FILES.contains(&entry);
            if !allowed {
                faults.extend(catalogue_fault(
                    NODE_UNIT_LAYOUT_CODE,
                    path,
                    (1, 1),
                    &format!("{unit_path}/{entry} is not a unit role file"),
                ));
            }
        } else if entry != UNIT_HELPERS_FOLDER {
            folders.insert(format!("{unit_path}/{entry}"));
        }
    }
    faults.extend(folders.iter().filter_map(|folder| {
        catalogue_fault(
            NODE_UNIT_LAYOUT_CODE,
            package_anchor(sources, folder)?,
            (1, 1),
            &format!("{folder}/ is not {UNIT_HELPERS_FOLDER}/"),
        )
    }));
    faults
}

/// Each node or factory declared outside a unit's `node.py`, and each check outside its unit's files.
pub(crate) fn declared_outside_unit_faults(project: &UnitProject<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    for file in &project.files {
        let home: Option<&str> = unit_file_name(file);
        for function in top_functions(file) {
            let outer: Option<Declared> = declared(function).map(|(what, _)| what);
            if let Some(fault) = misplaced(file, home, function, false) {
                faults.push(fault);
            }
            let in_factory: bool = outer == Some(Declared::Factory);
            faults.extend(
                function
                    .inner
                    .iter()
                    .filter_map(|inner| misplaced(file, home, inner, in_factory)),
            );
        }
    }
    faults
}

/// The fault for `function` when what it declares does not belong in a file named `home`.
fn misplaced(
    file: &UnitFile<'_>,
    home: Option<&str>,
    function: &UnitFunction,
    in_factory: bool,
) -> Option<Fault> {
    let (what, decorator) = declared(function)?;
    let at_home: bool = match what {
        Declared::Node(..) => home == Some(UNIT_NODE_FILE),
        Declared::Factory => matches!(home, Some(UNIT_NODE_FILE | UNIT_CHECKS_FILE)),
        Declared::Check => {
            home == Some(UNIT_CHECKS_FILE) || (home == Some(UNIT_NODE_FILE) && in_factory)
        }
    };
    if at_home {
        return None;
    }
    let destination: String = match what {
        Declared::Check => {
            format!("the {UNIT_CHECKS_FILE} of the unit folder whose node it checks")
        }
        _ => format!(
            "{PACKAGE_ROOT}/{}/{}/{UNIT_NODE_FILE}",
            domain(file),
            function.name
        ),
    };
    unit_fault(
        NODE_DECLARED_IN_UNIT_CODE,
        file.path,
        function.position,
        &format!(
            "@{} '{}' in {} belongs in {destination}",
            decorator.name, function.name, file.path
        ),
    )
}

/// The domain folder `file` sits in, or `<domain>` when it sits in no domain.
fn domain(file: &UnitFile<'_>) -> String {
    let [first, _, ..] = file.parts.as_slice() else {
        return "<domain>".to_owned();
    };
    if is_role_folder(first) {
        "<domain>".to_owned()
    } else {
        (*first).to_owned()
    }
}
