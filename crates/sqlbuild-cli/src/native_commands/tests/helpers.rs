use std::path::Path;

use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::{ArgvOutcome, ArgvSettings};
use crate::native_commands::_helpers::argv_host::{NativeArgvHost, namespace_defaults};

/// Parse `argv` onto the `CliNamespace` defaults with the native host.
pub(super) fn parse(argv: &[&str]) -> (ArgvOutcome, NativeArgvHost) {
    let argv: Vec<String> = argv.iter().map(|arg| (*arg).to_owned()).collect();
    let settings: ArgvSettings = ArgvSettings {
        columns: 100,
        use_color: false,
    };
    let mut host: NativeArgvHost = NativeArgvHost::default();
    let outcome: ArgvOutcome =
        parse_cli_arguments(&argv, namespace_defaults(), settings, &mut host);
    (outcome, host)
}

/// Create `directories` and write `files` beneath `root`.
pub(super) fn write_tree(root: &Path, directories: &[&str], files: &[(&str, &str)]) {
    for directory in directories {
        std::fs::create_dir_all(root.join(directory)).expect("create directory");
    }
    for (relative, contents) in files {
        std::fs::write(root.join(relative), contents).expect("write file");
    }
}

/// `sqb plan --json` output naming `count` changed models, each with `diff_line` as its query diff.
pub(super) fn plan_output(diff_line: &str, count: usize) -> Vec<u8> {
    let models: Vec<serde_json::Value> = (0..count)
        .map(|i| serde_json::json!({"name": format!("model_{i}"), "reason": "query_changed", "query_diff": [diff_line], "config_diff": ["+a"]}))
        .collect();
    serde_json::to_vec(&serde_json::json!({"selected_count": count, "models": models}))
        .expect("plan output")
}
