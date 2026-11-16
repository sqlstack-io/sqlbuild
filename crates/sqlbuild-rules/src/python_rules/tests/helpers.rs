use std::collections::BTreeSet;

use fensu_facts::extension::models::ProgramHandle;
use fensu_native::{NativeProjectPlane, NativeRuleContext, evaluate_core_rules};

use crate::engine::_helpers::memo::{evaluate_rows, prepare};
use crate::models::{EvaluateRequest, RowsRequest, RuleMetadata, RuleScope, RulesConfig};
use crate::python_rules::tests::test_types::CachedRuns;
use crate::rules::_helpers::dispatch::catalogue::{catalogue, select};

/// How many distinct `SQBRPYTHON` codes the catalogue holds.
pub(super) fn distinct_python_codes(rules: &[RuleMetadata]) -> usize {
    rules
        .iter()
        .filter(|rule| rule.code.starts_with("SQBRPYTHON"))
        .map(|rule| rule.code.as_str())
        .collect::<BTreeSet<&str>>()
        .len()
}

/// How many catalogue rules are project-wide Python rules.
pub(super) fn project_wide_python_rules(rules: &[RuleMetadata]) -> usize {
    rules
        .iter()
        .filter(|rule| {
            rule.scope == RuleScope::Python && rule.project_wide && rule.family == "python"
        })
        .count()
}

/// Whether selecting `select` runs the rule `code`.
pub(super) fn is_selected(selectors: &[String], code: &str) -> bool {
    let rules: Vec<RuleMetadata> = catalogue();
    select(&rules, Some(selectors), &[])
        .expect("valid selection")
        .iter()
        .any(|rule| rule.code == code)
}

/// The faults one Fensu core rule reports on a root-scope module, as code and line.
pub(super) fn core_rule_faults(source: &str, fensu_code: &str) -> Vec<(String, u32)> {
    let program: ProgramHandle = ProgramHandle::parse_source(source, 3, 12).expect("source parses");
    let context = NativeRuleContext {
        scope: "root".to_owned(),
        ..NativeRuleContext::default()
    };
    evaluate_core_rules(
        &program,
        &[fensu_code.to_owned()],
        &context,
        &NativeProjectPlane::new(Vec::new(), Vec::new()),
    )
    .expect("rule evaluates")
    .into_iter()
    .map(|fault| (fault.code, fault.line))
    .collect()
}

/// The speculation keys of a request selecting `select`, before and after its one Python module changes from `original` to `edited`.
pub(super) fn source_keys(select: &[&str], original: &str, edited: &str) -> (String, String) {
    let directory = tempfile::tempdir().expect("temporary project");
    let module = directory.path().join("python/orders/main.py");
    std::fs::create_dir_all(module.parent().expect("parent")).expect("folders");
    std::fs::write(&module, original).expect("module");
    let before: String = request_key(directory.path(), select);
    std::fs::write(&module, edited).expect("module");
    (before, request_key(directory.path(), select))
}

fn request_key(project_dir: &std::path::Path, select: &[&str]) -> String {
    let config: RulesConfig =
        serde_json::from_value(serde_json::json!({ "select": select })).expect("rules config");
    let request = EvaluateRequest {
        project_dir: project_dir.to_string_lossy().into_owned(),
        config,
        ..EvaluateRequest::default()
    };
    prepare(
        RowsRequest {
            request,
            memo: None,
            retain: false,
        },
        true,
    )
    .expect("request prepares")
    .key
    .expect("keyed")
}

/// The Python faults of a project holding `files`, under rules config `config`, as code, path, line and column.
pub(super) fn python_rule_faults(
    files: &[(&str, &str)],
    config: serde_json::Value,
) -> Result<Vec<(String, String, u64, u64)>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    for (path, contents) in files {
        let file = directory.path().join(path);
        std::fs::create_dir_all(file.parent().expect("parent"))
            .map_err(|error| error.to_string())?;
        std::fs::write(&file, contents).map_err(|error| error.to_string())?;
    }
    let mut config = config;
    config["cache"] = serde_json::json!({ "enabled": false });
    let request = EvaluateRequest {
        project_dir: directory.path().to_string_lossy().into_owned(),
        config: serde_json::from_value(config).map_err(|error| error.to_string())?,
        ..EvaluateRequest::default()
    };
    let evaluation = evaluate_rows(RowsRequest {
        request,
        memo: None,
        retain: false,
    })
    .map_err(|error| format!("{error:?}"))?;
    Ok(evaluation
        .faults
        .into_iter()
        .filter(|fault| fault.code.starts_with("SQBRPYTHON"))
        .map(|fault| (fault.code, fault.path, fault.line, fault.column))
        .collect())
}

/// Expected faults as the owned tuples `python_rule_faults` returns.
pub(super) fn owned_faults(faults: &[(&str, &str, u64, u64)]) -> Vec<(String, String, u64, u64)> {
    faults
        .iter()
        .map(|(code, path, line, column)| ((*code).to_owned(), (*path).to_owned(), *line, *column))
        .collect()
}

/// Python faults of two cached runs, `files` then `changes` under `changed_config`, and the second's cache use.
pub(super) fn cached_runs(
    files: &[(&str, &str)],
    config: serde_json::Value,
    changes: &[(&str, &str)],
    changed_config: serde_json::Value,
) -> Result<CachedRuns, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    write_files(directory.path(), files)?;
    let first = cached_run(directory.path(), config)?;
    write_files(directory.path(), changes)?;
    let second = cached_run(directory.path(), changed_config)?;
    Ok(CachedRuns {
        first_faults: python_codes(&first),
        second_faults: python_codes(&second),
        second_hits: second.cache_hits,
        second_misses: second.cache_misses,
    })
}

fn write_files<P: AsRef<str>, C: AsRef<str>>(
    root: &std::path::Path,
    files: &[(P, C)],
) -> Result<(), String> {
    for (path, contents) in files {
        let file = root.join(path.as_ref());
        std::fs::create_dir_all(file.parent().expect("parent"))
            .map_err(|error| error.to_string())?;
        std::fs::write(&file, contents.as_ref()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// A project of `units` small domain entry modules sharing one helper, plus `noisy_files` hooks of `functions` unannotated functions each.
pub(super) fn scaling_project(
    units: usize,
    noisy_files: usize,
    functions: usize,
) -> Vec<(String, String)> {
    let helper = (
        "python/utils/text/join.py".to_owned(),
        "\"\"\"Join text.\"\"\"\n\n\ndef joined(value: str) -> str:\n    \"\"\"Return the value.\"\"\"\n    return value\n".to_owned(),
    );
    let modules = (0..units).map(|unit| {
        (
            format!("python/domain_{}/unit_{unit}/main.py", unit % 50),
            format!(
                "\"\"\"Unit {unit}.\"\"\"\n\nfrom python.utils.text.join import joined\n\n\ndef run_{unit}(value: str) -> str:\n    \"\"\"Run unit {unit}.\"\"\"\n    return joined(value)\n"
            ),
        )
    });
    let noisy = (0..noisy_files).map(|file| {
        let body: String = (0..functions)
            .map(|function| {
                format!("def alert_{function}(ctx, reason):\n    print(ctx, reason)\n\n\n")
            })
            .collect();
        (format!("hooks/python/alert_{file}.py"), body)
    });
    std::iter::once(helper)
        .chain(modules)
        .chain(noisy)
        .collect()
}

/// A project of a chain of `units` node units over 50 domains, each node depending on the one before, plus `undescribed` unlinked units whose nodes have no description.
pub(super) fn unit_scaling_project(units: usize, undescribed: usize) -> Vec<(String, String)> {
    let head = unit_files(0, "", "description=\"Unit 0.\"");
    let chain = (1..units).flat_map(|unit| {
        let previous = unit - 1;
        unit_files(
            unit,
            &format!(
                "from {}.node import node_{previous}\n",
                unit_module(previous)
            ),
            &format!("depends_on=[node_{previous}], description=\"Unit {unit}.\""),
        )
    });
    let bare = (units..units + undescribed).flat_map(|unit| unit_files(unit, "", ""));
    let utils = (
        "python/utils/text/join.py".to_owned(),
        "def joined(value: str) -> str:\n    return value\n".to_owned(),
    );
    head.into_iter()
        .chain(chain)
        .chain(bare)
        .chain(std::iter::once(utils))
        .collect()
}

fn unit_module(unit: usize) -> String {
    format!("python.area{}.unit_{unit}", unit % 50)
}

/// The node, main and checks files of `unit`, its node importing `upstream` and declared with `arguments`.
fn unit_files(unit: usize, upstream: &str, arguments: &str) -> [(String, String); 3] {
    let module = unit_module(unit);
    let dir = module.replace('.', "/");
    [
        (
            format!("{dir}/node.py"),
            format!(
                "{upstream}from {module}.main import main\n\n\n@task({arguments})\ndef node_{unit}(ctx: TaskContext) -> None:\n    return main(ctx=ctx)\n"
            ),
        ),
        (
            format!("{dir}/main.py"),
            "from sqlbuild import PythonNodeResult\n\nfrom python.utils.text.join import joined\n\n\ndef main(ctx: TaskContext) -> PythonNodeResult:\n    return ctx.result(joined(\"\"))\n".to_owned(),
        ),
        (
            format!("{dir}/checks.py"),
            format!(
                "from {module}.node import node_{unit}\n\n\n@check(depends_on=[node_{unit}], description=\"Ran.\")\ndef ran_{unit}(ctx: CheckContext) -> bool:\n    return ctx.ok()\n"
            ),
        ),
    ]
}

/// Root modules calling into helper chains that each end in a request.
pub(super) fn purity_chain_project(
    roots: usize,
    chains: usize,
    depth: usize,
) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();
    for chain in 0..chains {
        for step in 1..depth {
            files.push((
                format!("python/utils/chain_{chain}/step_{previous}.py", previous = step - 1),
                format!(
                    "from python.utils.chain_{chain} import step_{step}\n\n\ndef run():\n    return step_{step}.run()\n"
                ),
            ));
        }
        files.push((
            format!(
                "python/utils/chain_{chain}/step_{last}.py",
                last = depth - 1
            ),
            "import requests\n\n\ndef run():\n    return requests.get(\"https://example.com\")\n"
                .to_owned(),
        ));
    }
    for root in 0..roots {
        files.push((
            format!("python/orders/export_{root}/constants.py"),
            format!(
                "from python.utils.chain_{chain} import step_0\n\nREGIONS = step_0.run()\n",
                chain = root % chains
            ),
        ));
    }
    files
}

/// The codes of the Python faults `files` produce under `config` with the cache off, and how long evaluation took.
pub(super) fn timed_python_codes(
    files: &[(String, String)],
    config: serde_json::Value,
) -> Result<(Vec<String>, std::time::Duration), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    write_files(directory.path(), files)?;
    let mut config = config;
    config["cache"] = serde_json::json!({ "enabled": false });
    let started = std::time::Instant::now();
    let evaluation = cached_run(directory.path(), config)?;
    let elapsed = started.elapsed();
    let codes = python_codes(&evaluation)
        .into_iter()
        .map(|(code, _)| code)
        .collect();
    Ok((codes, elapsed))
}

/// Hold while timing, so the scaling tests never measure each other's work.
pub(super) fn serial_timing() -> std::sync::MutexGuard<'static, ()> {
    static TIMING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TIMING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The median over interleaved `pairs` of the large project's time divided by the small one's.
pub(super) fn median_growth(
    small: &[(String, String)],
    large: &[(String, String)],
    config: &serde_json::Value,
    pairs: usize,
) -> f64 {
    let mut ratios: Vec<f64> = (0..pairs)
        .map(|_| pair_growth(small, large, config))
        .collect();
    ratios.sort_by(f64::total_cmp);
    ratios[ratios.len() / 2]
}

fn pair_growth(
    small: &[(String, String)],
    large: &[(String, String)],
    config: &serde_json::Value,
) -> f64 {
    let (_, small_elapsed) = timed_python_codes(small, config.clone()).expect("rules run");
    let (_, large_elapsed) = timed_python_codes(large, config.clone()).expect("rules run");
    large_elapsed.as_secs_f64() / small_elapsed.as_secs_f64()
}

fn cached_run(
    project_dir: &std::path::Path,
    config: serde_json::Value,
) -> Result<crate::models::RowsEvaluation, String> {
    let request = EvaluateRequest {
        project_dir: project_dir.to_string_lossy().into_owned(),
        config: serde_json::from_value(config).map_err(|error| error.to_string())?,
        ..EvaluateRequest::default()
    };
    evaluate_rows(RowsRequest {
        request,
        memo: None,
        retain: false,
    })
    .map_err(|error| format!("{error:?}"))
}

fn python_codes(evaluation: &crate::models::RowsEvaluation) -> Vec<(String, String)> {
    evaluation
        .faults
        .iter()
        .filter(|fault| fault.code.starts_with("SQBRPYTHON"))
        .map(|fault| (fault.code.clone(), fault.path.clone()))
        .collect()
}

/// Expected faults as owned code and path pairs.
pub(super) fn named_faults(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(code, path)| ((*code).to_owned(), (*path).to_owned()))
        .collect()
}
