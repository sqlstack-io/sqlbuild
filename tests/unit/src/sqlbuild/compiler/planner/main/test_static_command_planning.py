"""Focused command planning regressions."""

from __future__ import annotations

from dataclasses import replace
from datetime import UTC, datetime
from pathlib import Path
from unittest.mock import Mock

import pytest

from sqlbuild._native import (
    CompileModelConfig,
    Dialect,
    DiscoveredSeedFile,
    Fingerprint,
    PlannerRelationsContext,
    PlannerScope,
    PlannerSelection,
    PlanOutput,
    PlanReason,
    SeedPlanEntry,
    build_audit_command_plan,
    build_seed_command_plan,
    build_test_command_plan,
    compiled_model_record,
    live_reads,
    project_with,
    read_selected_seed_fingerprints,
    resolve_static_command_scope,
    resolve_static_relation_context,
)
from sqlbuild.compiler.compile.types import CompiledProjectView, CompiledSeedView
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import (
    PlannerTestAdapter,
    build_test_project,
)
from tests.unit.src.sqlbuild.compiler.planner.main._test_types import (
    StaticCommandPlanningTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (StaticCommandPlanningTestCase("focused audit projection", ("orders_audit",)),),
    ids=lambda case: case.description,
)
def test_given_incremental_attached_audit_when_planning_then_no_warehouse_method_is_called(
    test_case: StaticCommandPlanningTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project: CompiledProjectView = build_test_project(
        model_deps={"orders": ()},
        audit_model_source_deps={"orders": ()},
    )
    project = project_with(
        project=project,
        models=(
            replace(
                compiled_model_record(project.models[0]),
                config=CompileModelConfig(values={"materialized": "incremental", "cursor": "id"}),
            ),
        ),
    )
    adapter: PlannerTestAdapter = PlannerTestAdapter()
    for method_name in ("connect", "execute", "list_relations", "get_columns_for_relations"):
        monkeypatch.setattr(
            adapter,
            method_name,
            lambda *args, _method_name=method_name, **kwargs: pytest.fail(
                f"unexpected warehouse call: {_method_name}"
            ),
        )

    scope: PlannerScope = resolve_static_command_scope(
        project=project,
        selection=PlannerSelection(select=("orders",)),
    )
    relations: PlannerRelationsContext = resolve_static_relation_context(
        project=project,
        dialect=Dialect(adapter),
        scope=scope,
    )
    plan: PlanOutput = build_audit_command_plan(
        project=project,
        dialect=Dialect(adapter),
        scope=scope,
        relations=relations,
    )

    assert tuple(entry.name for entry in plan.audit_entries) == test_case.expected_names
    assert plan.audit_entries[0].attached_target_name == "orders"


@pytest.mark.parametrize(
    "test_case",
    (StaticCommandPlanningTestCase("focused SQL test projection", ("is_valid_order",)),),
    ids=lambda case: case.description,
)
def test_given_sql_test_with_function_dependency_when_planning_then_required_function_is_projected(
    test_case: StaticCommandPlanningTestCase,
) -> None:
    project: CompiledProjectView = build_test_project(
        model_deps={"orders": ("is_valid_order",)},
        sql_test_expected_model_names=("orders",),
        function_names=("is_valid_order",),
    )
    adapter: PlannerTestAdapter = PlannerTestAdapter()
    scope: PlannerScope = resolve_static_command_scope(
        project=project,
        selection=PlannerSelection(),
    )
    relations: PlannerRelationsContext = resolve_static_relation_context(
        project=project,
        dialect=Dialect(adapter),
        scope=scope,
    )

    plan: PlanOutput = build_test_command_plan(
        project=project,
        dialect=Dialect(adapter),
        scope=scope,
        relations=relations,
    )

    assert tuple(entry.name for entry in plan.test_entries) == ("test_models",)
    assert tuple(entry.name for entry in plan.function_entries) == test_case.expected_names


@pytest.mark.parametrize(
    "test_case",
    (StaticCommandPlanningTestCase("focused seed identity", expected_reason="no_change"),),
    ids=lambda case: case.description,
)
def test_given_selected_seed_when_planning_then_identity_and_reason_are_preserved(
    test_case: StaticCommandPlanningTestCase,
    tmp_path: Path,
) -> None:
    seed_path: Path = tmp_path / "countries.csv"
    seed_path.write_text("code,name\nGB,United Kingdom\n", encoding="utf-8")
    project: CompiledProjectView = build_test_project(seed_names=("countries",))
    project = project_with(
        project=project,
        seeds=(
            replace(
                project.seeds[0],
                seed_file=DiscoveredSeedFile(
                    file_path=seed_path,
                    relative_path=Path("sql/seeds/countries.csv"),
                ),
            ),
        ),
    )
    adapter: PlannerTestAdapter = PlannerTestAdapter()
    scope: PlannerScope = resolve_static_command_scope(
        project=project,
        selection=PlannerSelection(select=("seed:countries",)),
    )
    relations: PlannerRelationsContext = resolve_static_relation_context(
        project=project,
        dialect=Dialect(adapter),
        scope=scope,
    )

    first_plan: PlanOutput = build_seed_command_plan(
        project=project,
        scope=scope,
        relations=relations,
    )
    first_entry: SeedPlanEntry = first_plan.seed_entries[0]

    assert first_entry.reason == PlanReason.FIRST_RUN
    assert first_entry.fingerprint_version_hash
    assert '"rows":[["code","name"],["GB","United Kingdom"]]' in (
        first_entry.fingerprint_metadata_json
    )
    current: Fingerprint = Fingerprint(
        node_type="seed",
        node_name="countries",
        target_database=None,
        target_schema=None,
        target_name="countries",
        run_id="previous",
        definition_hash=first_entry.fingerprint_version_hash,
        version_hash=first_entry.fingerprint_version_hash,
        schema_fingerprint="",
        definition=first_entry.fingerprint_metadata_json,
        metadata_json=first_entry.fingerprint_metadata_json,
        ts=datetime.now(UTC),
    )

    current_plan: PlanOutput = build_seed_command_plan(
        project=project,
        scope=scope,
        relations=relations,
        fingerprints={"countries": current},
    )

    assert current_plan.seed_entries[0].reason.value == test_case.expected_reason


@pytest.mark.parametrize(
    "test_case",
    (StaticCommandPlanningTestCase("targeted seed state relation", ("_sqlbuild_fingerprints",)),),
    ids=lambda case: case.description,
)
def test_given_selected_seed_without_state_table_when_reading_reason_then_only_state_relation_is_inspected(
    test_case: StaticCommandPlanningTestCase,
    tmp_path: Path,
) -> None:
    seed_path: Path = tmp_path / "countries.csv"
    seed_path.write_text("code\nGB\n", encoding="utf-8")
    project: CompiledProjectView = build_test_project(seed_names=("countries",))
    seed: CompiledSeedView = replace(
        project.seeds[0],
        seed_file=DiscoveredSeedFile(
            file_path=seed_path,
            relative_path=Path("sql/seeds/countries.csv"),
        ),
        destination=replace(project.seeds[0].destination, schema="analytics"),
    )
    adapter: Mock = Mock()
    adapter.list_relations.return_value = ()
    adapter.render_qualified_name.return_value = "analytics._sqlbuild_fingerprints"
    connection: object = object()

    fingerprints: dict[str, Fingerprint] = read_selected_seed_fingerprints(
        reads=live_reads(adapter=adapter, connection=connection),
        seeds=(seed,),
    )

    assert fingerprints == {}
    adapter.list_relations.assert_called_once_with(
        connection=connection,
        database=None,
        schemas=("analytics",),
        names=test_case.expected_names,
    )
    adapter.execute.assert_not_called()


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
