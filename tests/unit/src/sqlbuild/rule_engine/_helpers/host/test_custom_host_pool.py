"""Custom rules split across host processes report exactly what one host reports."""

import time
from pathlib import Path

import pytest

from sqlbuild._native import (
    CustomHostObserver,
    CustomRulesOutcome,
    HostCancelledError,
    RulesError,
    rules_bulk_cache_path,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.rule_engine._helpers.host import custom_host
from tests.unit.src.sqlbuild.rule_engine._helpers.host._test_types import (
    AwaitInputsTestCase,
    AwaitPublishedInputsTestCase,
    HostCancellationTestCase,
    HostFailureTestCase,
    HostInputsTestCase,
    HostInterruptTestCase,
    HostPartitionTestCase,
    SingleHostInterruptTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.host.helpers import (
    child_processes,
    delayed_publish,
    evaluate_order_rules,
    failed_publish,
    failed_publish_without_directory,
    host_inputs_left,
    orders_project,
    record_signals,
    started_hosts,
    touch_files,
    use_hosts,
    write_order_rules,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.host.interrupts.helpers import (
    default_interrupt_handling,
    interrupt_when,
)


@pytest.mark.parametrize(
    "test_case",
    [
        HostPartitionTestCase(
            description="three hosts", model_count=7, hosts=3, expected_host_runs=3
        ),
        HostPartitionTestCase(
            description="more hosts than subjects", model_count=2, hosts=8, expected_host_runs=3
        ),
        HostPartitionTestCase(
            description="module state reruns the rule in one host",
            model_count=7,
            hosts=3,
            expected_host_runs=4,
            records_module_state=True,
        ),
        HostPartitionTestCase(
            description="cross-model duplicate detector reruns in one host",
            model_count=7,
            hosts=3,
            expected_host_runs=4,
            detects_duplicates=True,
        ),
        HostPartitionTestCase(
            description="uncached cross-model duplicate detector reruns in one host",
            model_count=7,
            hosts=3,
            expected_host_runs=4,
            detects_duplicates=True,
            cache_enabled=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_partitioned_hosts_when_evaluating_then_findings_and_cache_match_one_host(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostPartitionTestCase
) -> None:
    write_order_rules(
        root=tmp_path,
        records_module_state=test_case.records_module_state,
        detects_duplicates=test_case.detects_duplicates,
    )
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    cache_path: Path = rules_bulk_cache_path(project_dir=tmp_path, file_name="custom-rules.json")
    cache_glob: str = cache_path.relative_to(tmp_path).as_posix()
    cache_enabled: bool = test_case.cache_enabled

    single_hosts: CustomHostObserver = use_hosts(monkeypatch=monkeypatch, hosts=1)
    single: CustomRulesOutcome = evaluate_order_rules(
        project=project, root=tmp_path, cache_enabled=cache_enabled
    )
    single_warm: CustomRulesOutcome = evaluate_order_rules(
        project=project, root=tmp_path, cache_enabled=cache_enabled
    )
    single_launches: int = len(single_hosts.launches())
    single_cache: bytes = b"".join(path.read_bytes() for path in tmp_path.glob(cache_glob))
    cache_path.unlink(missing_ok=True)
    split_hosts: CustomHostObserver = use_hosts(monkeypatch=monkeypatch, hosts=test_case.hosts)
    split: CustomRulesOutcome = evaluate_order_rules(
        project=project, root=tmp_path, cache_enabled=cache_enabled
    )
    split_cold_launches: int = len(split_hosts.launches())
    split_cache: bytes = b"".join(path.read_bytes() for path in tmp_path.glob(cache_glob))
    split_warm: CustomRulesOutcome = evaluate_order_rules(
        project=project, root=tmp_path, cache_enabled=cache_enabled
    )

    assert split.findings == single.findings
    assert split_warm.findings == single_warm.findings == single.findings
    assert single_launches == 1 + int(not cache_enabled)
    assert split_cold_launches == test_case.expected_host_runs
    assert (split.cache_hits, split.cache_misses) == (single.cache_hits, single.cache_misses)
    assert (split_warm.cache_hits, split_warm.cache_misses) == (
        single_warm.cache_hits,
        single_warm.cache_misses,
    )
    assert split_cache == single_cache
    assert bool(single_cache) is cache_enabled
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        HostPartitionTestCase(
            description="edit after split", model_count=6, hosts=3, expected_host_runs=3
        )
    ],
    ids=lambda case: case.description,
)
def test_given_results_cached_by_split_hosts_when_rerunning_then_only_edited_subjects_rerun(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostPartitionTestCase
) -> None:
    write_order_rules(root=tmp_path)
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    edited: CompiledProjectView = orders_project(
        model_count=test_case.model_count, edits={"orders_003": "SELECT 2 AS order_id"}
    )
    hosts: CustomHostObserver = use_hosts(monkeypatch=monkeypatch, hosts=test_case.hosts)

    cold: CustomRulesOutcome = evaluate_order_rules(project=project, root=tmp_path)
    cold_launches: int = len(hosts.launches())
    warm: CustomRulesOutcome = evaluate_order_rules(project=project, root=tmp_path)
    warm_launches: int = len(hosts.launches()) - cold_launches
    after_edit: CustomRulesOutcome = evaluate_order_rules(project=edited, root=tmp_path)
    oracle: CustomRulesOutcome = evaluate_order_rules(
        project=edited, root=tmp_path, cache_enabled=False
    )

    assert cold_launches == test_case.expected_host_runs
    assert warm_launches == 0
    assert (warm.cache_hits, warm.cache_misses) == (cold.cache_misses, 0)
    assert warm.findings == cold.findings
    assert (after_edit.cache_hits, after_edit.cache_misses) == (cold.cache_misses - 1, 1)
    assert after_edit.findings == oracle.findings


@pytest.mark.parametrize(
    "test_case",
    [
        HostFailureTestCase(
            description="earliest failure lives on a later host",
            model_count=6,
            failing_models=("orders_001", "orders_003"),
            expected_failing_model="orders_001",
        ),
        HostFailureTestCase(
            description="one failing subject",
            model_count=6,
            failing_models=("orders_005",),
            expected_failing_model="orders_005",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_rule_on_split_hosts_when_evaluating_then_single_host_error_is_raised(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostFailureTestCase
) -> None:
    write_order_rules(root=tmp_path, failing=test_case.failing_models)
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)

    _ = use_hosts(monkeypatch=monkeypatch, hosts=1)
    with pytest.raises(RulesError) as single:
        _ = evaluate_order_rules(project=project, root=tmp_path)
    _ = use_hosts(monkeypatch=monkeypatch, hosts=3)
    with pytest.raises(RulesError) as split:
        _ = evaluate_order_rules(project=project, root=tmp_path)

    assert str(split.value) == str(single.value)
    assert f"cannot inspect {test_case.expected_failing_model}" in str(split.value)
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        HostCancellationTestCase(
            description="busy hosts stop at their next invocation",
            model_count=60,
            hosts=3,
            delay_seconds=2.0,
            expected_error_fragment="cannot inspect orders_000",
            expected_max_seconds=20.0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_host_when_others_are_busy_then_they_stop_before_later_invocations(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostCancellationTestCase
) -> None:
    write_order_rules(root=tmp_path, failing=("orders_000",), delay_seconds=test_case.delay_seconds)
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    hosts: CustomHostObserver = use_hosts(monkeypatch=monkeypatch, hosts=test_case.hosts)
    signals: list[tuple[int, int]] = record_signals(monkeypatch=monkeypatch)
    started: float = time.monotonic()

    with pytest.raises(RulesError) as raised:
        _ = evaluate_order_rules(project=project, root=tmp_path)

    assert time.monotonic() - started < test_case.expected_max_seconds
    assert test_case.expected_error_fragment in str(raised.value)
    assert len(hosts.launches()) == test_case.hosts + 1
    assert signals == []
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        HostInterruptTestCase(
            description="busy split hosts stop after an interrupt",
            model_count=60,
            hosts=3,
            delay_seconds=1.0,
            interrupt_at_hosts=3,
            expected_host_launches=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_busy_split_hosts_when_interrupted_then_hosts_stop_and_no_process_remains(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostInterruptTestCase
) -> None:
    started: Path = tmp_path / "started"
    started.mkdir()
    write_order_rules(root=tmp_path, delay_seconds=test_case.delay_seconds, started_dir=started)
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    children_before: frozenset[int] = child_processes()
    hosts: CustomHostObserver = use_hosts(monkeypatch=monkeypatch, hosts=test_case.hosts)

    with (
        default_interrupt_handling(),
        interrupt_when(lambda: started_hosts(started) >= test_case.interrupt_at_hosts) as sent,
        pytest.raises(KeyboardInterrupt),
    ):
        _ = evaluate_order_rules(project=project, root=tmp_path)

    assert sent.is_set()
    assert len(hosts.launches()) == test_case.expected_host_launches
    assert len(tuple(started.iterdir())) < test_case.model_count
    assert child_processes() <= children_before
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        HostInterruptTestCase(
            description="interrupt while the single-host fallback runs",
            model_count=30,
            hosts=3,
            delay_seconds=0.2,
            interrupt_at_hosts=4,
            expected_host_launches=4,
            failing=("orders_029",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_interrupt_while_hosts_are_busy_when_the_run_ends_then_the_interrupt_is_raised(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostInterruptTestCase
) -> None:
    started: Path = tmp_path / "started"
    started.mkdir()
    write_order_rules(
        root=tmp_path,
        delay_seconds=test_case.delay_seconds,
        started_dir=started,
        failing=test_case.failing,
    )
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    hosts: CustomHostObserver = use_hosts(monkeypatch=monkeypatch, hosts=test_case.hosts)
    outcomes: list[str] = []

    with default_interrupt_handling():
        try:
            with interrupt_when(
                lambda: started_hosts(started) >= test_case.interrupt_at_hosts
            ) as sent:
                try:
                    _ = evaluate_order_rules(project=project, root=tmp_path, cache_enabled=False)
                except RulesError:
                    outcomes.append("RulesError")
                for _ in range(100):
                    time.sleep(0.01)  # a pending interrupt is raised between bytecodes
        except KeyboardInterrupt:
            outcomes.append("KeyboardInterrupt")

    assert sent.is_set()
    assert len(hosts.launches()) == test_case.expected_host_launches
    assert outcomes[-1:] == ["KeyboardInterrupt"]


@pytest.mark.parametrize(
    "test_case",
    [
        SingleHostInterruptTestCase(
            description="one host stops after the invocation in flight",
            model_count=12,
            hosts=1,
            delay_seconds=0.3,
            interrupt_after=3,
            expected_host_launches=1,
            expected_max_started_after_interrupt=1,
        ),
        SingleHostInterruptTestCase(
            description="the single-host fallback stops after the invocation in flight",
            model_count=30,
            hosts=3,
            delay_seconds=0.2,
            interrupt_after=33,
            expected_host_launches=4,
            expected_max_started_after_interrupt=1,
            failing=("orders_029",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_single_host_run_when_interrupted_then_it_stops_after_the_invocation_in_flight(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: SingleHostInterruptTestCase
) -> None:
    started: Path = tmp_path / "started"
    started.mkdir()
    write_order_rules(
        root=tmp_path,
        delay_seconds=test_case.delay_seconds,
        started_dir=started,
        failing=test_case.failing,
    )
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    children_before: frozenset[int] = child_processes()
    hosts: CustomHostObserver = use_hosts(monkeypatch=monkeypatch, hosts=test_case.hosts)
    started_at_interrupt: list[int] = []

    def ready() -> bool:
        started_at_interrupt[:] = [len(tuple(started.iterdir()))]
        return started_at_interrupt[0] >= test_case.interrupt_after

    outcomes: list[str] = []

    with default_interrupt_handling():
        try:
            with interrupt_when(ready) as sent:
                try:
                    _ = evaluate_order_rules(project=project, root=tmp_path, cache_enabled=False)
                except RulesError:
                    outcomes.append("RulesError")
                for _ in range(100):
                    time.sleep(0.01)  # a pending interrupt is raised between bytecodes
        except KeyboardInterrupt:
            outcomes.append("KeyboardInterrupt")

    assert sent.is_set()
    assert outcomes == ["KeyboardInterrupt"]
    assert len(hosts.launches()) == test_case.expected_host_launches
    assert (
        len(tuple(started.iterdir())) - started_at_interrupt[0]
        <= test_case.expected_max_started_after_interrupt
    )
    assert child_processes() <= children_before
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        HostInputsTestCase(
            description="one host waits for a slow payload",
            model_count=4,
            hosts=1,
            write_delay_seconds=1.0,
            expected_max_seconds=60.0,
        ),
        HostInputsTestCase(
            description="split hosts wait for a slow payload",
            model_count=7,
            hosts=3,
            write_delay_seconds=1.0,
            expected_max_seconds=60.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_hosts_started_before_payload_when_payload_arrives_then_findings_are_unchanged(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostInputsTestCase
) -> None:
    write_order_rules(root=tmp_path)
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    oracle: CustomRulesOutcome = evaluate_order_rules(
        project=project, root=tmp_path, cache_enabled=False
    )
    hosts: CustomHostObserver = use_hosts(
        monkeypatch=monkeypatch,
        hosts=test_case.hosts,
        publish_inputs=delayed_publish(test_case.write_delay_seconds),
    )
    started: float = time.monotonic()

    delayed: CustomRulesOutcome = evaluate_order_rules(project=project, root=tmp_path)

    assert time.monotonic() - started < test_case.expected_max_seconds
    assert len(hosts.launches()) == test_case.hosts
    assert delayed.findings == oracle.findings
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        HostInputsTestCase(
            description="one waiting host is released",
            model_count=4,
            hosts=1,
            write_delay_seconds=0.5,
            expected_max_seconds=30.0,
        ),
        HostInputsTestCase(
            description="split waiting hosts are released",
            model_count=7,
            hosts=3,
            write_delay_seconds=0.5,
            expected_max_seconds=30.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_payload_write_failure_when_hosts_wait_then_original_error_is_raised_promptly(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostInputsTestCase
) -> None:
    write_order_rules(root=tmp_path)
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    hosts: CustomHostObserver = use_hosts(
        monkeypatch=monkeypatch,
        hosts=test_case.hosts,
        publish_inputs=failed_publish(test_case.write_delay_seconds),
    )
    started: float = time.monotonic()

    with pytest.raises(OSError, match="project payload disk is full"):
        _ = evaluate_order_rules(project=project, root=tmp_path)

    assert time.monotonic() - started < test_case.expected_max_seconds
    assert len(hosts.launches()) == test_case.hosts
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        HostInputsTestCase(
            description="one host released after its input folder vanished",
            model_count=4,
            hosts=1,
            write_delay_seconds=2.0,
            expected_max_seconds=1.0,
        ),
        HostInputsTestCase(
            description="split hosts released after their input folder vanished",
            model_count=7,
            hosts=3,
            write_delay_seconds=2.0,
            expected_max_seconds=1.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_input_folder_removed_when_payload_write_fails_then_original_error_returns_promptly(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: HostInputsTestCase
) -> None:
    write_order_rules(root=tmp_path)
    project: CompiledProjectView = orders_project(model_count=test_case.model_count)
    publish, failures = failed_publish_without_directory(test_case.write_delay_seconds)
    hosts: CustomHostObserver = use_hosts(
        monkeypatch=monkeypatch, hosts=test_case.hosts, publish_inputs=publish
    )

    with pytest.raises(OSError, match="project payload disk is full"):
        _ = evaluate_order_rules(project=project, root=tmp_path)

    assert time.monotonic() - failures[0] < test_case.expected_max_seconds
    assert len(hosts.launches()) == test_case.hosts
    assert not host_inputs_left(tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        AwaitInputsTestCase(
            description="parent exited before publishing",
            reported_parent_pid=1,
            existing_files=(),
            expected_error=RulesError,
            expected_max_seconds=1.0,
        ),
        AwaitInputsTestCase(
            description="run abandoned by its parent",
            reported_parent_pid=4242,
            existing_files=("project.abandoned",),
            expected_error=HostCancelledError,
            expected_max_seconds=1.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_waiting_host_when_parent_exits_or_run_is_abandoned_then_wait_stops_promptly(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: AwaitInputsTestCase
) -> None:
    touch_files(root=tmp_path, names=test_case.existing_files)
    monkeypatch.setattr(custom_host.os, "getppid", lambda: test_case.reported_parent_pid)
    started: float = time.monotonic()

    with pytest.raises(test_case.expected_error):
        custom_host.await_inputs(
            path=tmp_path / "project.payload",
            cancel_markers=(tmp_path / "project.abandoned",),
            parent_pid=4242,
        )

    assert time.monotonic() - started < test_case.expected_max_seconds


@pytest.mark.parametrize(
    "test_case",
    (
        AwaitPublishedInputsTestCase(
            description="payload published while the parent runs",
            reported_parent_pid=4242,
            existing_files=("project.payload",),
            expected_max_seconds=1.0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_published_payload_when_parent_still_runs_then_wait_returns(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, test_case: AwaitPublishedInputsTestCase
) -> None:
    touch_files(root=tmp_path, names=test_case.existing_files)
    monkeypatch.setattr(custom_host.os, "getppid", lambda: test_case.reported_parent_pid)
    started: float = time.monotonic()

    custom_host.await_inputs(
        path=tmp_path / "project.payload",
        cancel_markers=(tmp_path / "project.abandoned",),
        parent_pid=4242,
    )

    assert time.monotonic() - started < test_case.expected_max_seconds


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
