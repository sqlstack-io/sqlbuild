//! Run planned custom-rule invocations across isolated host processes and merge their results.

use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyBytes;
use serde_json::{Map, Value, json};
use sqlbuild_host::entries::constants::CUSTOM_HOST_LAUNCH_MODULE;
use sqlbuild_rules::custom_host_pool::constants::{
    CGROUP_ROOT, HOST_TIMEOUT_MILLIS, MIN_INVOCATIONS_PER_HOST, PROC_CGROUP,
};
use sqlbuild_rules::custom_host_pool::main::available_cores::available_cores;
use sqlbuild_rules::custom_host_pool::main::decode_host_response::decode_host_response;
use sqlbuild_rules::custom_host_pool::main::host_count::host_count;
use sqlbuild_rules::custom_host_pool::main::merge_host_slices::merge_host_slices;
use sqlbuild_rules::custom_host_pool::main::partition_host_plan::partition_host_plan;
use sqlbuild_rules::custom_host_pool::main::planned_invocations::planned_invocations;
use sqlbuild_rules::custom_host_pool::main::without_codes::without_codes;
use sqlbuild_rules::custom_host_pool::models::{HostRuleSubject, HostRun, HostSlice};
use sqlbuild_rules::custom_host_pool::types::{HostInvocation, HostPlan};
use sqlbuild_rules::engine::main::custom_host::run_custom_host_json;

use crate::rules::_helpers::custom::custom_host_observation::LaunchLog;
use crate::rules::_helpers::custom::custom_host_observation::ObservedHosts;
use crate::rules::_helpers::custom::custom_host_observation::before_publish;
use crate::rules::_helpers::custom::custom_host_observation::observed_hosts;
use crate::rules::_helpers::custom::custom_host_observation::record_launch;
use crate::rules::_helpers::custom::custom_host_payload::write_payload;
use crate::rules::constants::CUSTOM_HOST_ABANDONED_SUFFIX;
use crate::rules::constants::CUSTOM_HOST_CANCELLED_MARKER;
use crate::rules::constants::CUSTOM_HOST_RUNTIME_VERSION;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::types::ErrorId;

const HOST_INPUTS: [&str; 3] = ["target", "rules-cache", "host-inputs"];
/// How often a waiting run checks for interrupts and cancellation.
const SIGNAL_POLL: Duration = Duration::from_millis(100);
const INTERRUPTED_MESSAGE: &str = "custom rule hosts were interrupted";

static INPUT_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Set when the run that started these hosts was interrupted; hosts stop at their next invocation.
pub(crate) type HostCancel = Arc<AtomicBool>;

/// What every host of one run is told, before its plan and cancellation marker are added.
pub(crate) struct HostRequest<'a, 'py> {
    pub(crate) project: &'a Bound<'py, PyAny>,
    pub(crate) config: &'a Bound<'py, PyAny>,
    pub(crate) project_dir: &'a Path,
    pub(crate) dialect: &'a str,
    pub(crate) rules: &'a [(String, HostRuleSubject)],
    pub(crate) model_paths: &'a [String],
    pub(crate) track_reads: bool,
    pub(crate) verify_determinism: bool,
    pub(crate) cancel: &'a HostCancel,
}

/// One host's outcome: its slice, or the `RulesError` message it failed with.
type HostOutcome = Result<HostSlice, String>;
/// Whether a host finished, without its slice, so the waiter can cancel the rest.
type HostStatus = Result<(), String>;

/// Evaluate `plan` across host processes and merge their results in single-host order.
pub(crate) fn run_custom_hosts(
    py: Python<'_>,
    request: &HostRequest<'_, '_>,
    plan: &HostPlan,
) -> PyResult<HostRun> {
    let invocations: Vec<HostInvocation> =
        planned_invocations(plan, request.rules, request.model_paths);
    let observed: ObservedHosts = observed_hosts(py)?;
    let plans: Vec<HostPlan> = if request.verify_determinism {
        vec![plan.clone()]
    } else {
        let cores: usize = match observed.cores {
            Some(cores) => cores,
            None => cores(py)?,
        };
        let min_invocations_per_host: usize = observed
            .min_invocations_per_host
            .unwrap_or(MIN_INVOCATIONS_PER_HOST);
        partition_host_plan(
            plan,
            &invocations,
            host_count(invocations.len(), (cores, min_invocations_per_host)),
        )
    };
    let input_dir: PathBuf = HOST_INPUTS
        .iter()
        .fold(request.project_dir.to_path_buf(), |path, part| {
            path.join(part)
        });
    std::fs::create_dir_all(&input_dir)?;
    let input_dir: PathBuf = input_dir.canonicalize()?;
    let stem: String = format!("project-{}", uuid_hex(py)?);
    let input_path: PathBuf = input_dir.join(format!("{stem}.payload"));
    let abandoned: PathBuf = input_dir.join(format!("{stem}{}", CUSTOM_HOST_ABANDONED_SUFFIX));
    let mut payload: Map<String, Value> = Map::new();
    payload.insert("project_payload_path".to_owned(), json!(text(&input_path)));
    payload.insert("abandoned_marker".to_owned(), json!(text(&abandoned)));
    payload.insert("parent_pid".to_owned(), json!(std::process::id()));
    payload.insert(
        "project_dir".to_owned(),
        json!(text(&request.project_dir.canonicalize()?)),
    );
    payload.insert("dialect".to_owned(), json!(request.dialect));
    payload.insert(
        "verify_determinism".to_owned(),
        json!(request.verify_determinism),
    );
    payload.insert("track_reads".to_owned(), json!(request.track_reads));
    let launch: Launch = Launch::new(py, payload, observed.launches)?;
    let publish = || {
        before_publish(py, observed.publish_inputs.as_ref(), &input_path)?;
        write_inputs(&input_path, request)
    };
    let responses: PyResult<Vec<HostSlice>> = run_partitions(
        py,
        (&launch, plan, &plans),
        (&input_dir, request.cancel),
        (publish, &abandoned),
    );
    remove_file(&input_path);
    remove_file(&abandoned);
    Ok(merge_host_slices(responses?, &invocations))
}

/// The program, arguments and shared payload every host starts with.
struct Launch {
    program: String,
    arguments: Vec<String>,
    runtime_version: String,
    payload: Map<String, Value>,
    launches: Option<LaunchLog>,
}

impl Launch {
    fn new(
        py: Python<'_>,
        payload: Map<String, Value>,
        launches: Option<LaunchLog>,
    ) -> PyResult<Self> {
        Ok(Self {
            program: imported(py, "sys", "executable")?.extract()?,
            arguments: vec!["-m".to_owned(), CUSTOM_HOST_LAUNCH_MODULE.to_owned()],
            runtime_version: CUSTOM_HOST_RUNTIME_VERSION.to_owned(),
            payload,
            launches,
        })
    }

    /// Run one host for `spec`, recording the launch when the run is observed.
    fn run(&self, spec: &str) -> HostOutcome {
        record_launch(self.launches.as_ref(), spec);
        host_outcome(spec)
    }

    /// The host request for one plan and cancel marker; only split hosts detect module state.
    fn spec(&self, plan: &HostPlan, cancel_marker: &Path, detect_state: bool) -> String {
        let mut payload: Map<String, Value> = self.payload.clone();
        payload.insert("plan".to_owned(), json!(plan));
        payload.insert("cancel_marker".to_owned(), json!(text(cancel_marker)));
        payload.insert("detect_state".to_owned(), json!(detect_state));
        json!({
            "program": self.program,
            "arguments": self.arguments,
            "timeout_millis": HOST_TIMEOUT_MILLIS,
            "runtime_version": self.runtime_version,
            "payload": payload,
        })
        .to_string()
    }
}

/// Start hosts while the payload is written; failures and module state fall back to one host.
fn run_partitions<'py>(
    py: Python<'py>,
    (launch, plan, plans): (&Launch, &HostPlan, &[HostPlan]),
    (input_dir, cancel): (&Path, &HostCancel),
    (write_inputs, abandoned): (impl FnOnce() -> PyResult<()>, &Path),
) -> PyResult<Vec<HostSlice>> {
    let split: bool = plans.len() > 1;
    let outcomes: Vec<HostOutcome> = run_cancellable(
        py,
        (launch, plans, split),
        (input_dir, cancel),
        (write_inputs, abandoned),
    )?;
    if !split {
        return outcomes
            .into_iter()
            .map(|outcome| outcome.map_err(|message| rules_error(py, message)))
            .collect();
    }
    interrupted(py, cancel)?;
    if outcomes.iter().any(Result::is_err) {
        return single_host(py, (launch, plan), (input_dir, cancel), abandoned)
            .map(|slice| vec![slice]);
    }
    let slices: Vec<HostSlice> = outcomes.into_iter().filter_map(Result::ok).collect();
    let stateful: BTreeSet<String> = slices
        .iter()
        .flat_map(|slice| slice.stateful.iter().cloned())
        .collect();
    if stateful.is_empty() {
        return Ok(slices);
    }
    let rerun_plan: HostPlan = plan
        .iter()
        .map(|(code, subjects)| {
            let subjects: Option<Vec<String>> = if stateful.contains(code) {
                subjects.clone()
            } else {
                Some(Vec::new())
            };
            (code.clone(), subjects)
        })
        .collect();
    let rerun: HostSlice = single_host(py, (launch, &rerun_plan), (input_dir, cancel), abandoned)?;
    let mut merged: Vec<HostSlice> = slices
        .into_iter()
        .map(|slice| without_codes(slice, &stateful))
        .collect();
    merged.push(rerun);
    Ok(merged)
}

/// Run one host for `plan` from its written payload; an interrupt stops its next invocation.
fn single_host(
    py: Python<'_>,
    (launch, plan): (&Launch, &HostPlan),
    (input_dir, cancel): (&Path, &HostCancel),
    abandoned: &Path,
) -> PyResult<HostSlice> {
    let outcomes: Vec<HostOutcome> = run_cancellable(
        py,
        (launch, std::slice::from_ref(plan), false),
        (input_dir, cancel),
        (|| Ok(()), abandoned),
    )?;
    outcomes
        .into_iter()
        .next()
        .unwrap_or_else(|| Err("custom rule host returned no outcome".to_owned()))
        .map_err(|message| rules_error(py, message))
}

/// Run one host per plan with a fresh cancel marker that an interrupt or failed host sets.
fn run_cancellable(
    py: Python<'_>,
    (launch, plans, detect_state): (&Launch, &[HostPlan], bool),
    (input_dir, cancel): (&Path, &HostCancel),
    (write_inputs, abandoned): (impl FnOnce() -> PyResult<()>, &Path),
) -> PyResult<Vec<HostOutcome>> {
    let cancel_dir: PathBuf = unique_directory(input_dir, "hosts-")?;
    let marker: PathBuf = cancel_dir.join(CUSTOM_HOST_CANCELLED_MARKER);
    let specs: Vec<String> = plans
        .iter()
        .map(|partition| launch.spec(partition, &marker, detect_state))
        .collect();
    let outcomes: PyResult<Vec<HostOutcome>> = run_hosts(
        py,
        (launch, specs),
        (write_inputs, abandoned),
        (&marker, cancel),
    );
    let _removed: std::io::Result<()> = std::fs::remove_dir_all(&cancel_dir);
    outcomes
}

/// Run every host on its own thread while this thread publishes the payload they wait for.
fn run_hosts(
    py: Python<'_>,
    (launch, specs): (&Launch, Vec<String>),
    (write_inputs, abandoned): (impl FnOnce() -> PyResult<()>, &Path),
    (cancel_marker, cancel): (&Path, &HostCancel),
) -> PyResult<Vec<HostOutcome>> {
    std::thread::scope(|scope| {
        let (sender, finished): (Sender<HostStatus>, Receiver<HostStatus>) = channel();
        let handles: Vec<_> = specs
            .into_iter()
            .map(|spec| {
                let sender: Sender<HostStatus> = sender.clone();
                scope.spawn(move || {
                    let outcome: HostOutcome = launch.run(&spec);
                    let status: HostStatus = outcome.as_ref().map(drop).map_err(Clone::clone);
                    let _sent: Result<(), std::sync::mpsc::SendError<HostStatus>> =
                        sender.send(status);
                    outcome
                })
            })
            .collect();
        drop(sender);
        let published: PyResult<()> = write_inputs();
        if let Err(error) = published {
            if let Ok(()) = touch(abandoned) {
                py.detach(|| handles.into_iter().for_each(|handle| drop(handle.join())));
            }
            return Err(error);
        }
        let stopped: PyResult<()> =
            await_hosts(py, (finished, handles.len()), cancel_marker, cancel);
        let outcomes: Vec<HostOutcome> = py.detach(move || {
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .unwrap_or_else(|_| Err("custom rule host thread panicked".to_owned()))
                })
                .collect()
        });
        stopped.map(|()| outcomes)
    })
}

/// Wait until every host finished or one failed; an interrupt cancels the rest and is returned.
fn await_hosts(
    py: Python<'_>,
    (finished, count): (Receiver<HostStatus>, usize),
    cancel_marker: &Path,
    cancel: &HostCancel,
) -> PyResult<()> {
    let finished: Mutex<Receiver<HostStatus>> = Mutex::new(finished);
    let mut remaining: usize = count;
    while remaining > 0 {
        let received: Result<HostStatus, RecvTimeoutError> = py.detach(|| match finished.lock() {
            Ok(receiver) => receiver.recv_timeout(SIGNAL_POLL),
            Err(_) => Err(RecvTimeoutError::Disconnected),
        });
        match received {
            Ok(Ok(())) => remaining -= 1,
            Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => {
                cancel_hosts(cancel_marker);
                return Ok(());
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Err(error) = interrupted(py, cancel) {
                    cancel_hosts(cancel_marker);
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

/// Raise a pending interrupt, or report that the run that started these hosts was interrupted.
fn interrupted(py: Python<'_>, cancel: &HostCancel) -> PyResult<()> {
    py.check_signals()?;
    if cancel.load(Ordering::Acquire) {
        return Err(PyKeyboardInterrupt::new_err(INTERRUPTED_MESSAGE));
    }
    Ok(())
}

/// Stop hosts at their next invocation; a running invocation still ends or times out.
fn cancel_hosts(cancel_marker: &Path) {
    let _touched: std::io::Result<()> = touch(cancel_marker);
}

/// One host run decoded, or the message `RulesError` reports for it.
fn host_outcome(spec: &str) -> HostOutcome {
    let response: String = run_custom_host_json(spec)?;
    let value: Value = serde_json::from_str(&response).map_err(|error| error.to_string())?;
    decode_host_response(&value)
}

/// Publish the project and config payload atomically; hosts started earlier wait for it.
fn write_inputs(path: &Path, request: &HostRequest<'_, '_>) -> PyResult<()> {
    let parent: &Path = path.parent().unwrap_or(path);
    let partial: PathBuf = parent.join(format!(
        "writing-{}-{}.payload",
        std::process::id(),
        INPUT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let written: PyResult<()> = write_payload_file(&partial, request);
    if let Err(error) = written {
        remove_file(&partial);
        return Err(error);
    }
    std::fs::rename(&partial, path)?;
    Ok(())
}

/// Stream the payload into a new file at `path`, never holding it whole in memory.
fn write_payload_file(path: &Path, request: &HostRequest<'_, '_>) -> PyResult<()> {
    let file: BufWriter<File> = BufWriter::new(File::create(path)?);
    let mut file: BufWriter<File> = write_payload(file, request.project, request.config)?;
    file.flush()?;
    Ok(())
}

/// `pickle.dumps(value)`, for payload values only Python holds.
pub(crate) fn pickled<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    Ok(imported(value.py(), "pickle", "dumps")?
        .call1((value,))?
        .downcast_into()?)
}

/// Usable CPUs: the process CPU count capped by any cgroup CPU quota.
fn cores(py: Python<'_>) -> PyResult<usize> {
    let os: Bound<'_, PyAny> = py.import("os")?.into_any();
    let count: Option<usize> = if os.hasattr("process_cpu_count")? {
        os.call_method0("process_cpu_count")?.extract()?
    } else if os.hasattr("sched_getaffinity")? {
        Some(os.call_method1("sched_getaffinity", (0,))?.len()?)
    } else {
        os.call_method0("cpu_count")?.extract()?
    };
    Ok(available_cores(
        count.unwrap_or(1),
        (Path::new(CGROUP_ROOT), Path::new(PROC_CGROUP)),
    ))
}

fn uuid_hex(py: Python<'_>) -> PyResult<String> {
    imported(py, "uuid", "uuid4")?
        .call0()?
        .getattr("hex")?
        .extract()
}

/// A new directory below `parent` named `prefix` plus a unique suffix, as `mkdtemp` creates.
fn unique_directory(parent: &Path, prefix: &str) -> PyResult<PathBuf> {
    loop {
        let directory: PathBuf = parent.join(format!(
            "{prefix}{}-{}",
            std::process::id(),
            INPUT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
}

/// Create `marker` and its folder so hosts can see it.
fn touch(marker: &Path) -> std::io::Result<()> {
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(marker)
        .map(drop)
}

fn remove_file(path: &Path) {
    let _removed: std::io::Result<()> = std::fs::remove_file(path);
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn rules_error(py: Python<'_>, message: String) -> PyErr {
    new_plain_error(py, ErrorId::Rules, &message)
}
