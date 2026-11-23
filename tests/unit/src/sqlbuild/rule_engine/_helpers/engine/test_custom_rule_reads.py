"""Custom-rule results are reused only when every fact the invocation read is unchanged."""

import os
from pathlib import Path

import pytest

import sqlbuild._native as native_module
from sqlbuild._native import CustomRulesOutcome
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import (
    CustomRuleProjectFileTestCase,
    CustomRuleReadTrackingTestCase,
    CustomRuleUncacheableTestCase,
    NativeBuildIdentityTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.engine.helpers import (
    evaluate_cached_custom_rules,
    two_model_project,
    write_project_file_rule,
    write_rule,
)

_CUSTOMERS_SQL: str = "SELECT 1 AS customer_id"
_OWNERS_BEFORE: str = "owners: [customers]\n"
_OWNERS_AFTER: str = "owners: [orders]\n"
# Rule hosts report every filesystem read as changed since the rule observed it.
_STALE_OBSERVATION_SITECUSTOMIZE: str = """from sqlbuild.rule_engine._helpers.host import tracked_facts

_digest = tracked_facts._digest_or_none


def _stale(*, value=None, error=None):
    return _digest(value=("stale", value)) if error is None else _digest(error=error)


tracked_facts._digest_or_none = _stale
"""
_FLAG_ORDERS: str = (
    '    return [ctx.finding(subject=model)] if model.name == "orders" and {condition} else []'
)


@pytest.mark.parametrize(
    "test_case",
    (
        CustomRuleReadTrackingTestCase(
            description="own SQL read reuses the unedited subject",
            body="_ = ctx.sql.for_model(model).expanded.source\n"
            + _FLAG_ORDERS.format(condition="True"),
            expected_hits_after_edit=1,
        ),
        CustomRuleReadTrackingTestCase(
            description="reading every model SQL invalidates every subject",
            body="sources = [ctx.sql.for_model(item).expanded.source "
            "for item in ctx.project.models]\n" + _FLAG_ORDERS.format(condition="len(sources) > 1"),
            expected_hits_after_edit=0,
        ),
        CustomRuleReadTrackingTestCase(
            description="unchanged contracts of every model keep both subjects",
            body="enforced = [ctx.contracts.enforced(item) for item in ctx.project.models]\n"
            + _FLAG_ORDERS.format(condition="not any(enforced)"),
            expected_hits_after_edit=2,
        ),
        CustomRuleReadTrackingTestCase(
            description="changed contract of another model invalidates readers",
            body="enforced = [ctx.contracts.enforced(item) for item in ctx.project.models]\n"
            + _FLAG_ORDERS.format(condition="not any(enforced)"),
            expected_hits_after_edit=0,
            edited_customers_sql=_CUSTOMERS_SQL,
            edited_customers_config={"contract": "enforced"},
        ),
        CustomRuleReadTrackingTestCase(
            description="module-level memo makes the rule untracked",
            module_prelude="_MEMO: dict[str, object] = {}",
            body="if 'sql' not in _MEMO:\n"
            "        _MEMO['sql'] = [ctx.sql.for_model(item).expanded.source "
            "for item in ctx.project.models]\n"
            + _FLAG_ORDERS.format(condition="'SELECT 2' in ' '.join(_MEMO['sql'])"),
            expected_hits_after_edit=0,
        ),
        CustomRuleReadTrackingTestCase(
            description="cached helper makes the rule untracked",
            module_prelude=(
                "import functools\n\n"
                "@functools.cache\n"
                "def _sources(ctx: RuleContext) -> str:\n"
                "    return ' '.join(ctx.sql.for_model(item).expanded.source "
                "for item in ctx.project.models)\n"
            ),
            body=_FLAG_ORDERS.format(condition="'SELECT 2' in _sources(ctx)").strip(),
            expected_hits_after_edit=0,
        ),
        CustomRuleReadTrackingTestCase(
            description="context attribute state makes the rule untracked",
            body="ctx.seen = model.name\n" + _FLAG_ORDERS.format(condition="True"),
            expected_hits_after_edit=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_recorded_reads_when_one_model_changes_then_only_dependent_results_rerun(
    tmp_path: Path,
    test_case: CustomRuleReadTrackingTestCase,
) -> None:
    _ = write_rule(root=tmp_path, body=test_case.body, module_import=test_case.module_prelude)
    original: CompiledProjectView = two_model_project(
        customers_sql=_CUSTOMERS_SQL, customers_config={}
    )
    edited: CompiledProjectView = two_model_project(
        customers_sql=test_case.edited_customers_sql,
        customers_config=test_case.edited_customers_config,
    )

    cold: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=original, project_dir=tmp_path, cache_enabled=True
    )
    warm: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=original, project_dir=tmp_path, cache_enabled=True
    )
    after_edit: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=edited, project_dir=tmp_path, cache_enabled=True
    )
    oracle: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=edited, project_dir=tmp_path, cache_enabled=False
    )

    assert (cold.cache_hits, cold.cache_misses) == (0, 2)
    assert (warm.cache_hits, warm.cache_misses) == (2, 0)
    assert warm.findings == cold.findings
    assert (after_edit.cache_hits, after_edit.cache_misses) == (
        test_case.expected_hits_after_edit,
        2 - test_case.expected_hits_after_edit,
    )
    assert after_edit.findings == oracle.findings


@pytest.mark.parametrize(
    "test_case",
    (
        CustomRuleUncacheableTestCase(
            description="private SQL view state",
            body="_ = ctx.sql._models\n" + _FLAG_ORDERS.format(condition="True"),
        ),
        CustomRuleUncacheableTestCase(
            description="private column view state",
            body="_ = ctx.columns._models\n" + _FLAG_ORDERS.format(condition="True"),
        ),
        CustomRuleUncacheableTestCase(
            description="private graph view state",
            body="_ = ctx.graph._compiled_by_path\n" + _FLAG_ORDERS.format(condition="True"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_private_view_access_when_evaluating_repeatedly_then_rule_is_never_reused(
    tmp_path: Path,
    test_case: CustomRuleUncacheableTestCase,
) -> None:
    _ = write_rule(root=tmp_path, body=test_case.body)
    project: CompiledProjectView = two_model_project(
        customers_sql=_CUSTOMERS_SQL, customers_config={}
    )

    cold: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )
    warm: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )

    assert (cold.cache_hits, cold.cache_misses) == (0, 2)
    assert (warm.cache_hits, warm.cache_misses) == (
        test_case.expected_warm_hits,
        2 - test_case.expected_warm_hits,
    )
    assert warm.findings == cold.findings


@pytest.mark.parametrize(
    "test_case",
    (
        CustomRuleProjectFileTestCase(
            description="hidden file read through a cached helper",
            file_path=".config/owners.yaml",
            module_prelude=(
                "import functools\n\n"
                "@functools.cache\n"
                "def _owners(ctx: RuleContext) -> str:\n"
                "    return ctx.project.tree.read_text('.config/owners.yaml')\n"
            ),
            body=_FLAG_ORDERS.format(condition="'orders' in _owners(ctx)").strip(),
            expected_rerun_hits=0,
            expected_finding_paths=("sql/models/orders.sql",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_untracked_rule_reading_hidden_file_when_file_changes_then_rule_reruns(
    tmp_path: Path,
    test_case: CustomRuleProjectFileTestCase,
) -> None:
    owners: Path = write_project_file_rule(
        root=tmp_path,
        file_path=test_case.file_path,
        contents=_OWNERS_BEFORE,
        body=test_case.body,
        module_import=test_case.module_prelude,
    )
    project: CompiledProjectView = two_model_project(
        customers_sql=_CUSTOMERS_SQL, customers_config={}
    )

    cold: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )
    warm: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )
    owners.write_text(_OWNERS_AFTER, encoding="utf-8")
    after_edit: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )

    assert cold.findings == ()
    assert (warm.cache_hits, warm.cache_misses) == (2, 0)
    assert (after_edit.cache_hits, after_edit.cache_misses) == (
        test_case.expected_rerun_hits,
        2 - test_case.expected_rerun_hits,
    )
    assert tuple(finding.path.as_posix() for finding in after_edit.findings) == (
        test_case.expected_finding_paths
    )


@pytest.mark.parametrize(
    "test_case",
    (
        CustomRuleProjectFileTestCase(
            description="file edited after the host read it",
            file_path="config/owners.yaml",
            body=_FLAG_ORDERS.format(
                condition="'orders' in ctx.project.tree.read_text('config/owners.yaml')"
            ).strip(),
            expected_rerun_hits=1,
            expected_finding_paths=("sql/models/orders.sql",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_file_edited_while_rules_run_when_rerunning_then_new_contents_are_evaluated(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    test_case: CustomRuleProjectFileTestCase,
) -> None:
    owners: Path = write_project_file_rule(
        root=tmp_path,
        file_path=test_case.file_path,
        contents=_OWNERS_BEFORE,
        body=test_case.body,
        module_import=test_case.module_prelude,
    )
    project: CompiledProjectView = two_model_project(
        customers_sql=_CUSTOMERS_SQL, customers_config={}
    )
    site: Path = tmp_path.parent / f"{tmp_path.name}-site"
    site.mkdir(parents=True)
    _ = (site / "sitecustomize.py").write_text(_STALE_OBSERVATION_SITECUSTOMIZE, encoding="utf-8")
    monkeypatch.setenv("PYTHONPATH", os.pathsep.join((str(site), os.environ.get("PYTHONPATH", ""))))
    during_edit: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )
    monkeypatch.undo()
    _ = owners.write_text(_OWNERS_AFTER, encoding="utf-8")
    rerun: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )
    oracle: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=False
    )

    assert during_edit.findings == ()
    assert (rerun.cache_hits, rerun.cache_misses) == (
        test_case.expected_rerun_hits,
        2 - test_case.expected_rerun_hits,
    )
    assert rerun.findings == oracle.findings
    assert tuple(finding.path.as_posix() for finding in rerun.findings) == (
        test_case.expected_finding_paths
    )


@pytest.mark.parametrize(
    "test_case",
    [NativeBuildIdentityTestCase("custom results follow the native build", 2, 0)],
    ids=lambda case: case.description,
)
def test_given_custom_rule_cache_from_another_native_build_when_evaluating_then_rules_rerun(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    test_case: NativeBuildIdentityTestCase,
) -> None:
    _ = write_rule(root=tmp_path, body=_FLAG_ORDERS.format(condition="True").strip())
    project: CompiledProjectView = two_model_project(
        customers_sql=_CUSTOMERS_SQL, customers_config={}
    )

    cold: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )
    monkeypatch.setattr(native_module, "BUILD_IDENTITY", f"{native_module.BUILD_IDENTITY}-rebuilt")
    rebuilt: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )
    warm: CustomRulesOutcome = evaluate_cached_custom_rules(
        project=project, project_dir=tmp_path, cache_enabled=True
    )

    assert rebuilt.cache_misses == test_case.expected_rebuilt_evaluations
    assert warm.cache_misses == test_case.expected_warm_evaluations
    assert rebuilt.findings == warm.findings == cold.findings


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
