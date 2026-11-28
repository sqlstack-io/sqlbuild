//! Whether a target's connection is one it can hold models in: of the project's kind, without `http` settings.

use crate::effective::constants::HTTP_ONLY_KEYS;
use crate::effective::models::WarehouseConnection;

/// Why `connection` can't hold the target's models, or `None` when it can.
pub fn warehouse_connection_problem(connection: &WarehouseConnection<'_>) -> Option<String> {
    let used: String = match connection.name {
        Some(name) => format!("connection '{name}'"),
        None => "a connection".to_owned(),
    };
    if let Some(kind) = connection.kind
        && kind != connection.adapter
    {
        return Some(format!(
            "target '{}' uses {used} ({kind}), which can't hold models; this project's targets \
             need {} connections",
            connection.target, connection.adapter
        ));
    }
    let key: &str = connection
        .keys
        .iter()
        .find(|key| HTTP_ONLY_KEYS.contains(key))?;
    let place: String = match connection.name {
        Some(name) => format!("connections.{name}"),
        None => format!("the connection of target '{}'", connection.target),
    };
    Some(format!(
        "{place}: {key} is for http connections; a warehouse connection can't take it"
    ))
}
