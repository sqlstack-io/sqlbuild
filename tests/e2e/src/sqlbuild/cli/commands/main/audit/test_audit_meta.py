"""E2E coverage for structured audit meta through results, lifecycle events and JSON output."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.audit._test_types import (
    AuditMetaChangeE2ETestCase,
    AuditMetaRoundTripE2ETestCase,
    InvalidAuditMetaE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.audit.helpers import (
    AUDIT_META_ORDERS_HEADER,
    audit_completed_meta,
    create_legacy_audit_results_table,
    json_asset_statuses,
    json_check_meta,
    leave_audit_results_table_absent,
    plan_reasons,
    prepare_audit_meta_project,
    stored_audit_identities,
    stored_audit_meta,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_DECLARED_META: dict[str, object] = {
    "valid_order_rate": {
        "region": "emea",
        "tiers": ["gold", "silver"],
        "weight": 1.5,
        "flags": {"late": False},
    },
    "positive_amount": {"segment": "core", "reference": "orders-quality", "rank": 1},
    "not_null": {"owner": "fulfillment"},
    "known_region": None,
}
_INVALID_FACTORY: str = """from datetime import date
from sqlbuild.audits import AuditCase, audit_factory


@audit_factory
def order_quality():
    return [
        AuditCase(
            name="positive_amount",
            definition="expression_is_true",
            arguments={"expression": "amount > 0"},
            meta={"since": date(2020, 1, 1)},
        )
    ]
"""
_INVALID_HEADER: str = """MODEL (description "Test model orders.", materialized table,
  audits [expression_is_true (expression "amount > 0", meta "emea")]);

SELECT 1 AS amount
"""


@pytest.mark.parametrize(
    "test_case",
    (
        AuditMetaRoundTripE2ETestCase(
            "fresh results table", leave_audit_results_table_absent, _DECLARED_META
        ),
        AuditMetaRoundTripE2ETestCase(
            "results table without meta column", create_legacy_audit_results_table, _DECLARED_META
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_audit_meta_when_building_then_meta_reaches_results_events_and_json(
    test_case: AuditMetaRoundTripE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_audit_meta_project(
        tmp_path=tmp_path, measurement_options=AUDIT_META_ORDERS_HEADER
    )
    test_case.prepare_results_table(project_dir)
    event_path: Path = tmp_path / "events.jsonl"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--json"),
        project_dir=project_dir,
        env={"ORDERS_EVENT_PATH": str(event_path)},
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert stored_audit_meta(project_dir) == test_case.expected_meta
    assert audit_completed_meta(event_path) == test_case.expected_meta
    assert json_check_meta(result.stdout) == test_case.expected_meta


@pytest.mark.parametrize(
    "test_case",
    (
        AuditMetaChangeE2ETestCase(
            "meta-only edit matches a description-only edit",
            original_header_fragment=AUDIT_META_ORDERS_HEADER,
            meta_only_fragment=AUDIT_META_ORDERS_HEADER.replace('"emea"', '"europe"'),
            description_only_fragment=AUDIT_META_ORDERS_HEADER.replace(
                "Valid order rate", "Valid order share"
            ),
            expected_meta_after_change={
                "region": "europe",
                "tiers": ["gold", "silver"],
                "weight": 1.5,
                "flags": {"late": False},
            },
            expected_plan_reasons={"orders": "no_change"},
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_meta_only_change_when_rebuilding_then_outcomes_and_reruns_match_description_change(
    test_case: AuditMetaChangeE2ETestCase,
    tmp_path: Path,
) -> None:
    statuses: dict[str, dict[str, str]] = {}
    baselines: dict[str, dict[str, tuple[object, ...]]] = {}
    rebuilt: dict[str, dict[str, tuple[object, ...]]] = {}
    project_dirs: dict[str, Path] = {}
    reasons: dict[str, dict[str, str]] = {}
    for variant, fragment in (
        ("meta", test_case.meta_only_fragment),
        ("description", test_case.description_only_fragment),
    ):
        project_dir: Path = prepare_audit_meta_project(
            tmp_path=tmp_path / variant, measurement_options=test_case.original_header_fragment
        )
        first: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--json"),
            project_dir=project_dir,
            env={"ORDERS_EVENT_PATH": str(tmp_path / f"{variant}-first.jsonl")},
        )
        assert first.returncode == 0, first.stdout + first.stderr
        baselines[variant] = stored_audit_identities(project_dir)
        model_path: Path = project_dir / "sql" / "models" / "orders.sql"
        model_path.write_text(
            model_path.read_text(encoding="utf-8").replace(
                test_case.original_header_fragment, fragment
            ),
            encoding="utf-8",
        )
        reasons[variant] = plan_reasons(project_dir)
        second: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build", "--json"),
            project_dir=project_dir,
            env={"ORDERS_EVENT_PATH": str(tmp_path / f"{variant}-second.jsonl")},
        )
        assert second.returncode == 0, second.stdout + second.stderr
        statuses[variant] = json_asset_statuses(second.stdout)
        rebuilt[variant] = stored_audit_identities(project_dir)
        project_dirs[variant] = project_dir

    assert reasons["meta"] == reasons["description"] == test_case.expected_plan_reasons
    assert statuses["meta"] == statuses["description"]
    assert rebuilt["meta"] == baselines["meta"]
    assert rebuilt["description"] == baselines["description"]
    assert (
        stored_audit_meta(project_dirs["meta"])["valid_order_rate"]
        == test_case.expected_meta_after_change
    )


@pytest.mark.parametrize(
    "test_case",
    (
        InvalidAuditMetaE2ETestCase(
            "factory meta with a date",
            "python/nodes/factories/order_quality.py",
            _INVALID_FACTORY,
            (
                "AuditCase 'positive_amount' meta must map string keys to JSON values",
                "meta['since'] is not JSON-serialisable",
            ),
        ),
        InvalidAuditMetaE2ETestCase(
            "header meta that is not a mapping",
            "sql/models/orders.sql",
            _INVALID_HEADER,
            ("'meta' must be a mapping of JSON values",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_non_json_meta_when_compiling_then_compile_error_names_the_value(
    test_case: InvalidAuditMetaE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_audit_meta_project(
        tmp_path=tmp_path, measurement_options=AUDIT_META_ORDERS_HEADER
    )
    (project_dir / test_case.relative_path).write_text(test_case.contents, encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode != 0, output
    assert [fragment in output for fragment in test_case.expected_fragments] == [True] * len(
        test_case.expected_fragments
    ), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
