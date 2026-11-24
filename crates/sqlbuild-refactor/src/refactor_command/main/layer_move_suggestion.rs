//! Suggest the one-step `sqb mv` when a rename fails only on the layer-folder rules.

use crate::refactor_command::_helpers::layer_folders::{folder_of, mirrored_folder, name_layer};
use crate::refactor_command::_helpers::quoting::command_line;
use crate::refactor_command::constants::{
    FOLDER_PLACEHOLDER, LAYER_FOLDER_RULE_CODES, LAYER_FOLDERS, PROJECT_DIR_OPTION, SQL_FILE_SUFFIX,
};
use crate::refactor_command::models::{LayerMove, RefactorDiagnostic};
use crate::refactoring::constants::MODELS_ROOT;
use crate::refactoring::models::{FileChange, RefactorOperation, RefactorPlan};

/// The `sqb mv` that renames and moves in one step, or none for any other failure.
#[must_use]
pub fn layer_move_suggestion(
    plan: &RefactorPlan,
    diagnostics: &[RefactorDiagnostic],
    (project_dir, windows): (Option<&str>, bool),
) -> Option<LayerMove> {
    let renamed: &FileChange = plan.changes.iter().find(|change| change.moved())?;
    if plan.request.operation != RefactorOperation::RenameModel
        || !plan.batch.is_empty()
        || diagnostics.is_empty()
        || diagnostics.iter().any(|diagnostic| {
            !LAYER_FOLDER_RULE_CODES.contains(&diagnostic.code.as_str())
                || diagnostic.path.as_deref() != Some(renamed.path.as_str())
        })
    {
        return None;
    }
    let new_name: &str = &plan.request.new_name;
    let layer_folder: Option<&[&str]> =
        name_layer(new_name).and_then(|layer| folder_of(&LAYER_FOLDERS, layer));
    let mirrored: Option<(Vec<String>, Vec<String>)> = layer_folder.and_then(|layer_folder| {
        mirrored_folder(
            &renamed.original_path,
            name_layer(&plan.request.model_name),
            layer_folder,
        )
    });
    let mut folders: Vec<String> = match &mirrored {
        Some((head, tail)) => head.iter().chain(tail).cloned().collect(),
        None => vec![MODELS_ROOT.to_owned(), FOLDER_PLACEHOLDER.to_owned()],
    };
    folders.push(format!("{new_name}{SQL_FILE_SUFFIX}"));
    let mut arguments: Vec<String> = vec![
        "sqb".to_owned(),
        "mv".to_owned(),
        plan.request.model_name.clone(),
        folders.join("/"),
    ];
    if let Some(project_dir) = project_dir {
        arguments.push(PROJECT_DIR_OPTION.to_owned());
        arguments.push(project_dir.to_owned());
    }
    Some(LayerMove {
        new_name: new_name.to_owned(),
        folder: mirrored.map(|(head, _)| head.join("/")),
        layer_folder: layer_folder.map(|folder| folder.join("/")),
        command: command_line(&arguments, windows),
    })
}
