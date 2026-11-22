use sqlbuild_orchestration::platform::models::LocationWorkScope;
use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;

use sqlbuild_orchestration::warehouse_metadata::models::{MetadataBatch, WarehouseStateLocation};
use uuid::Uuid;

use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::{ArgvOutcome, ArgvSettings};
use crate::native_commands::_helpers::command::argv_host::{NativeArgvHost, namespace_defaults};
use crate::native_commands::_helpers::command::paths::location_watch;
use crate::native_commands::_helpers::location::execution::{
    acknowledge_metadata_page, settle_publication,
};
use crate::native_commands::_helpers::location::plan_reuse::remember_plan;
use crate::native_commands::models::{
    LocationMetadataPage, LocationMetadataSync, LocationServe, PlanKey, PlanReuse, PlanToKeep,
};
use crate::native_commands::types::LocationPlan;
use sqlbuild_orchestration::platform::main::work::connect_location_work::connect_location_work;
use sqlbuild_orchestration::platform::models::{
    ConnectionCheckTrigger, LocationWorkClient, LocationWorkPoll,
};

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

/// Acknowledges `pages` complete, empty pages, each finishing the kind it was read for.
pub(super) fn acknowledge_complete_pages(
    state: LocationMetadataSync,
    pages: usize,
) -> LocationMetadataSync {
    (0..pages).fold(state, |mut state, _| {
        state.pending = Some(LocationMetadataPage {
            batch: MetadataBatch {
                kind: state.kind(),
                session_id: Uuid::nil(),
                generation: 1,
                records: Vec::new(),
                scan: None,
                last_page: false,
            },
            next_source_after: None,
        });
        acknowledge_metadata_page(state)
    })
}

/// One state location, for metadata sync tests.
pub(super) fn one_location() -> Vec<WarehouseStateLocation> {
    vec![WarehouseStateLocation {
        database: Some("analytics".into()),
        schema: "state".into(),
    }]
}

/// A location in `directory` whose `sqb debug` reports `wakes_compute` and whose every history read fails, published to an unreachable server: its options, script, work channel and the sync the publication starts.
pub(super) fn published_location(
    directory: &Path,
    wakes_compute: bool,
) -> (
    (LocationServe, std::path::PathBuf),
    (LocationWorkClient, LocationWorkPoll),
    LocationMetadataSync,
) {
    let script = directory.join(format!("location-{wakes_compute}"));
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ncase \" $* \" in *\" debug \"*) echo '{{\"history_reads_wake_compute\": \
             {wakes_compute}, \"local_files\": [], \"missing_secrets\": 0}}'; exit 0;; esac\n\
             while read line; do echo '{{\"failed\":{{\"reason\":\"permission denied\",\"page_size\":false}}}}'; done\n"
        ),
    )
    .expect("location script");
    let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&script, permissions).expect("executable");
    let options = LocationServe {
        project_dir: directory.to_path_buf(),
        server_url: "http://127.0.0.1:1".into(),
        location_id: Uuid::nil(),
        target: "development".into(),
        image_id: None,
        metadata_reconcile_seconds: 3600,
        safety_sync_seconds: 1800,
        dev_events: false,
    };
    let client = connect_location_work((&options.server_url, "token"), options.location_id)
        .expect("work client");
    let poll = LocationWorkPoll {
        session_id: Uuid::nil(),
        target: options.target.clone(),
        scope: LocationWorkScope::Standard,
    };
    let (_, metadata, _) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(settle_publication(
            (&options, &script, ("token", Uuid::nil())),
            ((one_location(), 1), true, ConnectionCheckTrigger::Publish),
            (location_watch(directory), None),
            std::future::pending(),
        ))
        .expect("no control error")
        .expect("not stopped");
    let mut state = metadata.expect("a connected publication starts a sync");
    state.begin_sync(chrono::Utc::now());
    state.idle = false;
    ((options, script), (client, poll), state)
}

/// The plan key of a `--changed` plan of `select`.
pub(super) fn reuse_key(select: &[&str]) -> PlanKey {
    PlanKey {
        argv: vec!["plan".into(), "--json".into(), "--changed".into()],
        select: select.iter().map(|name| (*name).to_owned()).collect(),
        consumption: true,
    }
}

/// A finished plan document, or a failed plan's error report.
pub(super) fn finished_plan(failed: bool) -> LocationPlan {
    let documents = [
        json!({
            "models": [{"name": "orders"}],
            "timings": {"planned_from": "synced_state", "total_seconds": 4.2,
                "steps": [{"name": "Detecting changes", "seconds": 4.2}]},
        }),
        json!({"error": {"code": "plan_failed", "message": "no plan"}}),
    ];
    (documents[usize::from(failed)].clone(), Default::default())
}

/// The bytes `finished_plan(false)` takes when kept: its document's and diffs' JSON text.
pub(super) fn finished_plan_bytes() -> usize {
    let (document, diffs) = finished_plan(false);
    serde_json::to_string(&document).expect("document").len()
        + serde_json::to_string(&diffs).expect("diffs").len()
}

/// The time the reuse tests plan at.
pub(super) fn reuse_clock() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5)
        .single()
        .expect("time")
}

/// A plan of `select` to keep under `generation` and `version`, started at `reuse_clock`.
pub(super) fn plan_to_keep(generation: i64, version: &str, select: &[&str]) -> PlanToKeep {
    PlanToKeep {
        generation,
        version: version.to_owned(),
        key: reuse_key(select),
        planned_at: reuse_clock(),
    }
}

/// `count` finished plans of `model_<n>` under generation 1 and `copy-1`, oldest first, within `limits`.
pub(super) fn kept_plans(count: usize, limits: (usize, usize)) -> Mutex<Option<PlanReuse>> {
    let reuse: Mutex<Option<PlanReuse>> = Mutex::new(None);
    let (document, diffs) = finished_plan(false);
    for index in 0..count {
        remember_plan(
            (&reuse, limits),
            plan_to_keep(1, "copy-1", &[format!("model_{index}").as_str()]),
            (&document, &diffs),
        );
    }
    reuse
}

/// The file `orders_plan` is defined in, in the discovered-set tests.
pub(super) const PLAN_FILE: &str = "python/plan/node.py";

/// The stamps of `paths`, the ones in `edited` with another size.
pub(super) fn stamps(
    paths: &[&str],
    edited: &[&str],
) -> Vec<sqlbuild_cache::project_snapshot::models::PathStamp> {
    paths
        .iter()
        .map(|path| sqlbuild_cache::project_snapshot::models::PathStamp {
            relative_path: (*path).to_owned(),
            kind: "file",
            size: 1 + u64::from(edited.contains(path)),
            mtime_ns: 0,
            ctime_ns: 0,
            inode: 0,
            link: None,
        })
        .collect()
}

/// A published load whose `orders_plan`, checked every 15 minutes and defined in `PLAN_FILE`, has `ids`.
pub(super) fn discovered_load(ids: &[&str]) -> crate::native_commands::models::LocationLoad {
    use sqlbuild_orchestration::discovered_sets::main::record_attempt::record_attempt;
    use sqlbuild_orchestration::discovered_sets::models::{DiscoveredItem, DiscoveredResult};
    let at: DateTime<Utc> = Utc
        .with_ymd_and_hms(2026, 11, 21, 9, 0, 0)
        .single()
        .expect("a valid time");
    let result = DiscoveredResult {
        items: ids
            .iter()
            .map(|id| DiscoveredItem {
                id: (*id).to_owned(),
                depends_on: Vec::new(),
                args: std::collections::BTreeMap::new(),
            })
            .collect(),
        tags: std::collections::BTreeMap::new(),
        max_parallel: None,
    };
    let record = sqlbuild_orchestration::snapshots::models::SnapshotRecord {
        snapshot_id: Uuid::nil(),
        location_id: Uuid::nil(),
        target: "dev".to_owned(),
        generation: 1,
        manifest_digest: String::new(),
        published_at: at,
    };
    crate::native_commands::models::LocationLoad {
        published: Some((record, None, Vec::new())),
        rules: None,
        discovered: [(
            "orders_plan".to_owned(),
            record_attempt(None, Ok(result), (at, Some(900))),
        )]
        .into(),
        discover_files: [("orders_plan".to_owned(), PLAN_FILE.to_owned())].into(),
        error: None,
    }
}

/// The reply of a compile that failed, with the sets it had discovered by then, if any.
pub(super) fn failed_load(discovered: bool) -> crate::native_commands::models::LocationLoad {
    let mut load = discovered_load(&["a"]);
    load.published = None;
    load.discover_files.clear();
    load.discovered.retain(|_, _| discovered);
    load
}
