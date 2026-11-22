//! Create a playground project from the packaged templates.

use crate::workspace::_helpers::playground::write_template;
use crate::workspace::constants::PLAYGROUND_TEMPLATES;
use crate::workspace::errors::WorkspaceError;
use crate::workspace::models::PlaygroundRequest;
use sqlbuild_core::init_command::main::ignore_env_file::ignore_env_file;

/// `create_playground_project`: refuse an existing target or unknown template, then copy.
pub fn create_playground(request: &PlaygroundRequest) -> Result<(), WorkspaceError> {
    if request.target_dir.exists() {
        return Err(WorkspaceError::user(
            "C701",
            format!(
                "playground target already exists: {}",
                request.target_dir.display()
            ),
            Some("choose a new directory or remove the existing one"),
        ));
    }
    if !PLAYGROUND_TEMPLATES.contains(&request.template.as_str()) {
        return Err(WorkspaceError::user(
            "C703",
            format!("unknown playground template: {}", request.template),
            Some(&format!(
                "choose one of: {}",
                PLAYGROUND_TEMPLATES.join(", ")
            )),
        ));
    }
    write_template(
        &request.templates_root,
        &request.target_dir,
        &request.template,
    )?;
    Ok(ignore_env_file(&request.target_dir)?)
}
