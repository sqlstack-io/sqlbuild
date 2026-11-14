//! Read `--config` once before any store opens; the replaced flags fail with where their settings went.

use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Namespace;
use crate::native_commands::_helpers::decision::text;
use crate::server_config::constants::REPLACED_SERVER_FLAGS;
use crate::server_config::models::ServerConfig;

pub(crate) fn read_server_config(namespace: &Namespace) -> Result<ServerConfig, ControlError> {
    if let Some((_, flag, section)) = REPLACED_SERVER_FLAGS
        .iter()
        .find(|(destination, _, _)| text(namespace, destination).is_some())
    {
        return Err(ControlError::Invalid(format!(
            "{flag} was replaced by --config: put its settings in the {section} section of \
             server.toml and start the server with --config <path>"
        )));
    }
    let Some(path) = text(namespace, "control_config") else {
        return Ok(ServerConfig::default());
    };
    let document = std::fs::read_to_string(&path)
        .map_err(|error| ControlError::Invalid(format!("could not read {path}: {error}")))?;
    toml::from_str(&document).map_err(|error: toml::de::Error| {
        let line = error
            .span()
            .map_or(1, |span| document[..span.start].matches('\n').count() + 1);
        ControlError::Invalid(format!(
            "{path} is not a valid server configuration at line {line}: {}",
            error.message().trim_end()
        ))
    })
}
