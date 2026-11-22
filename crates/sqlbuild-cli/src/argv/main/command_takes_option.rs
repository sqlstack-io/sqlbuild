//! Whether one command's parser defines an option.

use crate::argv::_helpers::table::command_parser;
use crate::argv::models::ParserSpec;

/// Whether the command at `path`, such as `["rules", "run"]`, defines an option written to `dest`.
#[must_use]
pub fn command_takes_option(path: &[&str], dest: &str) -> bool {
    let Some((command, nested)) = path.split_first() else {
        return false;
    };
    let Ok(mut parser) = command_parser(command) else {
        return false;
    };
    for name in nested {
        let child: Option<&ParserSpec> = parser
            .actions
            .iter()
            .flat_map(|action| &action.parsers)
            .find(|(subcommand, _)| subcommand == name)
            .map(|(_, child)| child);
        let Some(child) = child else {
            return false;
        };
        parser = child;
    }
    parser
        .actions
        .iter()
        .any(|action| action.dest.as_deref() == Some(dest))
}
