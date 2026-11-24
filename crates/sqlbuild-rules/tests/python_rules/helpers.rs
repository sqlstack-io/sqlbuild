use sqlbuild_rules::build_identity::main::install_build_identity::install_build_identity;
use sqlbuild_rules::engine::main::evaluate_rows::evaluate_rows;
use sqlbuild_rules::models::{EvaluateRequest, RowsRequest};

/// Counts the heap allocations of this test binary, which unlike time do not depend on the machine.
#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

/// A shared helper, `modules` modules that import it, and one hook file of 150 functions per 100.
pub(crate) fn importing_modules(modules: usize) -> Vec<(String, String)> {
    let helper = (
        "python/nodes/shared/text.py".to_owned(),
        "\"\"\"Shared text.\"\"\"\n\n\ndef trimmed(value: str) -> str:\n    \"\"\"Return the value.\"\"\"\n    return value\n".to_owned(),
    );
    std::iter::once(helper)
        .chain((0..modules).map(|module| {
            (
                format!("python/nodes/area_{}/module_{module}/entry.py", module % 40),
                format!(
                    "\"\"\"Module {module}.\"\"\"\n\nfrom python.shared.text import trimmed\n\n\ndef handle_{module}(value: str) -> str:\n    \"\"\"Handle module {module}.\"\"\"\n    return trimmed(value)\n"
                ),
            )
        }))
        .chain((0..modules / 100).map(|hook| {
            let body: String = (0..150)
                .map(|function| format!("def notify_{function}(ctx, reason):\n    print(ctx, reason)\n\n\n"))
                .collect();
            (format!("sql/hooks/python/notify_{hook}.py"), body)
        }))
        .collect()
}

/// The codes found over `files` and the heap allocations and reallocations the evaluation made.
pub(crate) fn counted_codes(
    files: &[(String, String)],
    config: &serde_json::Value,
) -> (Vec<String>, usize) {
    install_build_identity("0123456789abcdef").expect("the test identity installs");
    let directory = tempfile::tempdir().expect("project directory");
    for (path, contents) in files {
        let file = directory.path().join(path);
        std::fs::create_dir_all(file.parent().expect("parent")).expect("module directory");
        std::fs::write(&file, contents).expect("module file");
    }
    let mut config = config.clone();
    config["cache"] = serde_json::json!({ "enabled": false });
    let request = RowsRequest {
        request: EvaluateRequest {
            project_dir: directory.path().to_string_lossy().into_owned(),
            config: serde_json::from_value(config).expect("rules config"),
            ..EvaluateRequest::default()
        },
        memo: None,
        retain: false,
    };
    let region = stats_alloc::Region::new(ALLOCATOR);
    let evaluation = evaluate_rows(request).expect("rules run");
    let change = region.change();
    let codes = evaluation
        .faults
        .into_iter()
        .map(|fault| fault.code)
        .collect();
    (codes, change.allocations + change.reallocations)
}
