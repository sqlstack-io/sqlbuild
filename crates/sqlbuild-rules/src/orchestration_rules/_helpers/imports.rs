//! A sensor condition imported from another project module, followed to what it selects.

use std::collections::BTreeSet;

use sqlbuild_core::text::main::decode_text::decode_text;
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_pysource::orchestration_modules::main::module_binding_condition::module_binding_condition;
use sqlbuild_pysource::orchestration_modules::models::{
    ConditionSource, LiteralArgument, SourcePosition,
};

use crate::orchestration_rules::constants::{INIT_MODULE, MODULE_SUFFIX};

/// The project files and Python version imported conditions are read with.
#[derive(Debug)]
pub(crate) struct ImportReader<'a> {
    pub(crate) tree: &'a ProjectTree,
    pub(crate) python_version: (u8, u8),
}

/// What `source`, read in the module at `path`, selects once imports of project names are followed; `None` when it selects no nodes.
pub(crate) fn resolved_condition(
    reader: &ImportReader<'_>,
    path: &str,
    source: Option<ConditionSource>,
) -> Option<ConditionSource> {
    let mut followed: BTreeSet<(String, String)> = BTreeSet::new();
    let mut current: (String, Option<ConditionSource>) = (path.to_owned(), source);
    loop {
        let (
            from,
            Some(ConditionSource::Imported {
                module,
                level,
                name,
                position,
            }),
        ) = current
        else {
            return current.1;
        };
        let unresolvable = |what: String| Some(ConditionSource::Unresolvable { what, position });
        let shown: String = format!("{}{module}", ".".repeat(level as usize));
        let Some(file) = module_file(reader.tree, &from, &module, level) else {
            return unresolvable(format!(
                "`{name}` is imported from `{shown}`, which is not a project module"
            ));
        };
        if !followed.insert((file.clone(), name.clone())) {
            return unresolvable(format!("`{name}` is imported in a cycle through `{file}`"));
        }
        let bytes: Vec<u8> = match std::fs::read(reader.tree.absolute(&file)) {
            Ok(bytes) => bytes,
            Err(failure) => return unresolvable(format!("`{file}` cannot be read: {failure}")),
        };
        let contents: String = match decode_text(&bytes) {
            Ok(contents) => contents,
            Err(_undecodable) => return unresolvable(format!("`{file}` is not valid UTF-8")),
        };
        let next: Option<ConditionSource> =
            module_binding_condition(&contents, reader.python_version, &name);
        current = match next {
            Some(ConditionSource::Selection(selection)) => {
                return Some(ConditionSource::Selection(LiteralArgument {
                    value: selection.value,
                    position,
                }));
            }
            Some(ConditionSource::Unresolvable { what, .. }) => {
                return unresolvable(format!("in `{file}`, {what}"));
            }
            Some(imported @ ConditionSource::Imported { .. }) => {
                (file, Some(with_position(imported, position)))
            }
            None => return None,
        };
    }
}

/// `imported` reported where the sensor first named it.
fn with_position(imported: ConditionSource, at: SourcePosition) -> ConditionSource {
    match imported {
        ConditionSource::Imported {
            module,
            level,
            name,
            ..
        } => ConditionSource::Imported {
            module,
            level,
            name,
            position: at,
        },
        other => other,
    }
}

/// The project file `module` names, imported from the file at `from` with `level` leading dots.
fn module_file(tree: &ProjectTree, from: &str, module: &str, level: u32) -> Option<String> {
    let mut base: Vec<&str> = Vec::new();
    if level > 0 {
        base = from.split('/').collect();
        base.pop();
        for _ in 1..level {
            base.pop()?;
        }
    }
    base.extend(module.split('.').filter(|part| !part.is_empty()));
    let stem: String = base.join("/");
    [
        format!("{stem}{MODULE_SUFFIX}"),
        format!("{stem}/{INIT_MODULE}"),
    ]
    .into_iter()
    .find(|candidate| !stem.is_empty() && tree.exists(candidate))
}
