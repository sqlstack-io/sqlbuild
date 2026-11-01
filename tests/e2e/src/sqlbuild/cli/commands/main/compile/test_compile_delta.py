"""Delta compiles must print exactly what a full compile of the same edit prints."""

from __future__ import annotations

import shutil
from functools import partial
from pathlib import Path

import pytest

from scripts.delta_compile_series._helpers.outcomes import full_compile_difference
from scripts.delta_compile_series.constants import (
    DELTA_COMPILE_LINE,
    DELTA_VERIFIED_LINE,
    DOUBLE_CAST,
    DOUBLE_COLUMN,
    FLOAT_CAST,
    FLOAT_COLUMN,
    MISMATCH_EXIT_CODE,
    PROJECT_CONFIG,
)
from scripts.delta_compile_series.main.run_delta_compile_sequence import (
    run_delta_compile_sequence,
)
from scripts.delta_compile_series.main.run_delta_compile_series import run_delta_compile_series
from scripts.delta_compile_series.main.write_delta_compile_project import (
    write_delta_compile_project,
)
from scripts.delta_compile_series.models import (
    DeltaCompileOutcome,
    DeltaCompileSequence,
    DeltaCompileSeries,
)
from scripts.delta_compile_series.types import SequenceEdit
from sqlbuild._native import DELTA_COMPILE_DISABLE_ENV_VAR, DELTA_COMPILE_VERIFY_ENV_VAR
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DeltaCompileFullCopyTestCase,
    DeltaCompileOptOutTestCase,
    DeltaCompileProjectShapeTestCase,
    DeltaCompileSequenceTestCase,
    DeltaCompileSeriesTestCase,
    DeltaCompileVerifyTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    replace_project_text,
    run_delta_compile,
    run_full_compile,
    store_shaped_project,
    without_compile_timings,
)

_MODEL_COUNT: int = 300
_JSON_COMPILE: tuple[str, ...] = ("compile", "--json")
_TEXT_COMPILE: tuple[str, ...] = ("compile",)
_UNTYPED_ADAPTER: str = "sqlserver"
_SMALL_MODEL_COUNT: int = 30
_SMALL_MODEL_ROOT: str = "models/sales000/intermediate/clean"
_ANALYSED_MODEL: str = f"{_SMALL_MODEL_ROOT}/batch00000/sales000__int_clean__orders_00001.sql"
_SMALL_EDITED_MODEL: str = f"{_SMALL_MODEL_ROOT}/batch00001/sales000__int_clean__orders_00005.sql"
_RESHAPED_MODEL: str = f"{_SMALL_MODEL_ROOT}/batch00002/sales000__int_clean__orders_00010.sql"
_SMALL_EDITED_TEST: str = (
    "tests/unit/sales000/intermediate/clean/batch00001/"
    "test_sales000_int_clean_orders_00005__preserves_order_id.sql"
)
_EXPECTED_MODEL_CTE: str = "__expected__sales000__int_clean__orders_00005 AS"
_RETARGETED_MODEL_CTE: str = "__expected__sales000__int_clean__orders_00020 AS"
_BINDING_ERROR_EDIT: tuple[str, str, str] = (
    _SMALL_EDITED_MODEL,
    "COALESCE(b.amount, ",
    "COALESCE(b.missing_col, ",
)
_BODY_EDIT: tuple[str, str, str] = (
    _SMALL_EDITED_MODEL,
    "COALESCE(b.amount, ",
    "COALESCE(b.amount, 1 + ",
)
_PATH_DEFAULT_EDIT: tuple[str, str, str] = (
    "sqlbuild_project.toml",
    "[defaults]",
    '[path_defaults."sales000/intermediate/clean/batch00000"]\nmaterialized = "table"\n\n'
    "[defaults]",
)
_REUSE_ROOT: str = "target/cache/compiler-native-v1/project-reuse-v3"
_PIVOT_ROOT: str = f"{_SMALL_MODEL_ROOT}/batch00009"
_CATEGORY_AMOUNTS: tuple[str, str] = (
    f"{_PIVOT_ROOT}/order_category_amounts.sql",
    """MODEL (
  description "Order amounts per category.",
  materialized table,
  contract enforced,
  columns (
    customer_id (type INTEGER, description "Customer identifier."),
    category (type VARCHAR, description "Order category."),
    amount (type DOUBLE, description "Order amount."),
  ),
);

SELECT CAST(b.id AS INTEGER) AS customer_id, CAST('books' AS VARCHAR) AS category,
  CAST(b.amount AS DOUBLE) AS amount
FROM __ref("sales000__int_clean__orders_00001") AS b
""",
)
_CATEGORY_PIVOT_PATH: str = f"{_PIVOT_ROOT}/order_category_pivot.sql"
_CATEGORY_PIVOT: tuple[str, str] = (
    _CATEGORY_PIVOT_PATH,
    """MODEL (
  description "Order amounts pivoted per category.",
  materialized table,
  contract enforced,
  columns (customer_id (type INTEGER, description "Customer identifier.")),
  dynamic_columns (
    category_amounts (
      pivot_column category,
      value_column amount,
      aggregate MAX,
      type DOUBLE
    )
  ),
);

PIVOT __ref("order_category_amounts")
ON category
USING MAX(amount)
GROUP BY customer_id
""",
)
_ORDER_QUALITY_FACTORY: tuple[str, str] = (
    "python/factories/quality.py",
    """from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory


@audit_factory
def order_quality():
    return [
        AuditCase(
            name="amount_present",
            definition="not_null",
            arguments={"column": "amount"},
            severity=AuditSeverity.ERROR,
        ),
    ]
""",
)
_ATTACH_ORDER_QUALITY: tuple[str, str, str] = (
    _ANALYSED_MODEL,
    "MODEL (description",
    "MODEL (audit_factories [order_quality], description",
)
_RULE_HARNESS: str = """import pytest
from sqlbuild.rules.testing import RuleCase, evaluate_rule
from rules.{module} import {function}

CASES = (
    RuleCase(description="plain model", source="MODEL (); SELECT 1 AS id", expected_finding_count=0),
)


@pytest.mark.parametrize("test_case", CASES, ids=lambda case: case.description)
def test_given_model_when_checking_then_reports_expected_findings(test_case):
    result = evaluate_rule(rule={function}, test_case=test_case)
    assert result.finding_count == test_case.expected_finding_count
"""
_MODEL_SQL_RULE: tuple[str, str] = (
    "rules/order_sql.py",
    """from sqlbuild.rules import Finding, Model, RuleContext, rule


@rule(
    code="XSQBRBENCH002",
    message="Order models must not leave TODO markers",
    remediation="Resolve the TODO.",
)
def order_todo_markers(*, model: Model, ctx: RuleContext) -> list[Finding]:
    authored = ctx.sql.for_model(model).authored.source
    return [ctx.finding(subject=model)] if "TODO" in authored else []
""",
)
_FOLDER_RULE: tuple[str, str] = (
    "rules/order_folders.py",
    """from sqlbuild.rules import Finding, Model, RuleContext, rule


@rule(
    code="XSQBRBENCH003",
    message="Order models must share their folder with another model",
    remediation="Group the model with related models.",
)
def order_folder_siblings(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if not model.name.endswith("_00005"):
        return []
    siblings = ctx.project.tree.resources_under(model.path.parent.as_posix())
    return [ctx.finding(subject=model)] if len(siblings) < 2 else []
""",
)
# Each custom Rule needs a harness case, or the stored compile fails its Rules and the next is full.
_MODEL_SQL_RULE_CASES: tuple[str, str] = (
    "tests/test_rule_order_sql.py",
    _RULE_HARNESS.format(module="order_sql", function="order_todo_markers"),
)
_FOLDER_RULE_CASES: tuple[str, str] = (
    "tests/test_rule_order_folders.py",
    _RULE_HARNESS.format(module="order_folders", function="order_folder_siblings"),
)
# Built-in Rules cannot parse PIVOT models, which would fail the stored compile's Rules.
_BUILT_IN_RULES_OFF: tuple[str, str, str] = (
    "sqlbuild_project.toml",
    'select = ["SQBR", "SQLCLEAN", "XSQBR"]',
    'select = ["XSQBR"]',
)
_RULE_READ_MORE: str = "delta compile fell back: a custom rule read more than the changed models"
_FAMILY_TYPE_EDIT: tuple[str, str, str] = (_CATEGORY_PIVOT_PATH, "type DOUBLE", "type VARCHAR")
_FAMILIES_CHANGED: str = "delta compile fell back: a model's dynamic column families changed"
_EXAMPLE_TRACKED_VARIABLE: str = "SQLBUILD_ORDERS_EXAMPLE"
_TRACKED_VARIABLE_ADDED: str = (
    "delta compile fell back: the set of tracked environment variables changed"
)
_EDITED_MODEL: str = (
    "models/sales000/intermediate/clean/batch00000/sales000__int_clean__orders_00003.sql"
)


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaCompileSeriesTestCase(description="json_report", seed=11, json_output=True, steps=12),
        DeltaCompileSeriesTestCase(description="text_report", seed=12, json_output=False, steps=12),
        DeltaCompileSeriesTestCase(
            description="long_json_series", seed=13, json_output=True, steps=20
        ),
        DeltaCompileSeriesTestCase(
            description="partial_checks_text_report",
            seed=14,
            json_output=False,
            steps=12,
            adapter=_UNTYPED_ADAPTER,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seeded_edit_series_when_compiling_each_edit_then_deltas_match_full_compiles(
    test_case: DeltaCompileSeriesTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "dense_orders"
    write_delta_compile_project(
        project_dir=project_dir, model_count=_MODEL_COUNT, adapter=test_case.adapter
    )
    args: tuple[str, ...] = (_TEXT_COMPILE, _JSON_COMPILE)[int(test_case.json_output)]

    series: DeltaCompileSeries = run_delta_compile_series(
        project_dir=project_dir,
        seed=test_case.seed,
        steps=test_case.steps,
        compile_project=partial(
            run_delta_compile,
            project_dir=project_dir,
            args=args,
            env={DELTA_COMPILE_VERIFY_ENV_VAR: "1"},
        ),
    )

    assert series.unexpected == test_case.expected_unexpected_outcomes, series.steps
    assert series.deltas >= test_case.expected_minimum_deltas, series.steps


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaCompileSeriesTestCase(description="text_report", seed=15, json_output=False, steps=12),
        DeltaCompileSeriesTestCase(
            description="partial_checks_json_report",
            seed=16,
            json_output=True,
            steps=12,
            adapter=_UNTYPED_ADAPTER,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seeded_edit_series_when_compiling_each_edit_then_output_matches_full_compiles_of_copies(
    test_case: DeltaCompileSeriesTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "dense_orders"
    write_delta_compile_project(
        project_dir=project_dir, model_count=_MODEL_COUNT, adapter=test_case.adapter
    )
    args: tuple[str, ...] = (_TEXT_COMPILE, _JSON_COMPILE)[int(test_case.json_output)]

    series: DeltaCompileSeries = run_delta_compile_series(
        project_dir=project_dir,
        seed=test_case.seed,
        steps=test_case.steps,
        compile_project=partial(run_delta_compile, project_dir=project_dir, args=args, env={}),
        compile_in_full=partial(run_full_compile, args=args),
    )

    assert series.unexpected == test_case.expected_unexpected_outcomes, series.steps
    assert series.deltas >= test_case.expected_minimum_deltas, series.steps


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaCompileFullCopyTestCase(
            description="a model without SQL analysis keeps its partial-check notice",
            stored_edits=((_ANALYSED_MODEL, "MODEL (", "MODEL (sql_analysis false, "),),
            edits=(_BODY_EDIT,),
            expected_delta_line=True,
        ),
        DeltaCompileFullCopyTestCase(
            description="an invalid model makes its unchanged dependents partial",
            edits=(_BINDING_ERROR_EDIT,),
            expected_delta_line=True,
        ),
        DeltaCompileFullCopyTestCase(
            description="an invalid model makes its unchanged dependents partial in JSON",
            edits=(_BINDING_ERROR_EDIT,),
            expected_delta_line=True,
            json_output=True,
        ),
        DeltaCompileFullCopyTestCase(
            description="a dialect without type checks reports every model partial",
            edits=(_BODY_EDIT,),
            expected_delta_line=True,
            json_output=True,
            adapter=_UNTYPED_ADAPTER,
        ),
        DeltaCompileFullCopyTestCase(
            description="a path default matching only unchanged models",
            stored_edits=(_PATH_DEFAULT_EDIT,),
            edits=(_BODY_EDIT,),
            expected_delta_line=True,
        ),
        DeltaCompileFullCopyTestCase(
            description="a SQL test expecting another model falls back after its round",
            edits=((_SMALL_EDITED_TEST, _EXPECTED_MODEL_CTE, _RETARGETED_MODEL_CTE),),
            expected_delta_line=False,
        ),
        DeltaCompileFullCopyTestCase(
            description="a shape change printed once over several rounds",
            edits=(
                (_RESHAPED_MODEL, DOUBLE_COLUMN, FLOAT_COLUMN),
                (_RESHAPED_MODEL, DOUBLE_CAST, FLOAT_CAST),
                (_RESHAPED_MODEL, DOUBLE_CAST, FLOAT_CAST),
            ),
            expected_delta_line=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_stored_compile_when_compiling_an_edit_then_it_prints_what_a_full_compile_prints(
    test_case: DeltaCompileFullCopyTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "dense_orders"
    write_delta_compile_project(
        project_dir=project_dir, model_count=_SMALL_MODEL_COUNT, adapter=test_case.adapter
    )
    args: tuple[str, ...] = (_TEXT_COMPILE, _JSON_COMPILE)[int(test_case.json_output)]
    for relative_path, old, new in test_case.stored_edits:
        replace_project_text(project_dir, relative_path, old, new)
    _ = run_delta_compile(project_dir=project_dir, args=args, env={})
    for relative_path, old, new in test_case.edits:
        replace_project_text(project_dir, relative_path, old, new)
    copy: Path = tmp_path / "full_copy"
    _ = shutil.copytree(project_dir, copy, symlinks=True)

    edited: DeltaCompileOutcome = run_delta_compile(project_dir=project_dir, args=args, env={})

    assert (DELTA_COMPILE_LINE in edited.stderr) == test_case.expected_delta_line, edited.stderr
    assert (
        full_compile_difference(
            outcome=edited,
            full=run_full_compile(copy, args=args),
            project_dir=project_dir,
            full_project_dir=copy,
        )
        == test_case.expected_difference
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaCompileProjectShapeTestCase(
            description="a model with dynamic column families",
            files=(_CATEGORY_AMOUNTS, _CATEGORY_PIVOT),
            stored_edits=(_BUILT_IN_RULES_OFF,),
            edits=(_BODY_EDIT,),
            expected_delta_line=True,
        ),
        DeltaCompileProjectShapeTestCase(
            description="an audit factory attached only by an unchanged model",
            files=(_ORDER_QUALITY_FACTORY,),
            stored_edits=(_ATTACH_ORDER_QUALITY,),
            edits=(_BODY_EDIT,),
            expected_delta_line=True,
        ),
        DeltaCompileProjectShapeTestCase(
            description="a model-subject custom rule reading the edited model's SQL",
            files=(_MODEL_SQL_RULE, _MODEL_SQL_RULE_CASES),
            edits=(_BODY_EDIT,),
            expected_delta_line=True,
        ),
        DeltaCompileProjectShapeTestCase(
            description="a model-subject custom rule listing models under a folder",
            files=(_FOLDER_RULE, _FOLDER_RULE_CASES),
            edits=(_BODY_EDIT,),
            expected_delta_line=False,
        ),
        DeltaCompileProjectShapeTestCase(
            description="an edit to dynamic column families compiles in full",
            files=(_CATEGORY_AMOUNTS, _CATEGORY_PIVOT),
            stored_edits=(_BUILT_IN_RULES_OFF,),
            edits=(_FAMILY_TYPE_EDIT,),
            expected_delta_line=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_shape_when_compiling_an_edit_then_it_prints_what_a_full_compile_prints(
    test_case: DeltaCompileProjectShapeTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "dense_orders"
    write_delta_compile_project(project_dir=project_dir, model_count=_SMALL_MODEL_COUNT)
    store_shaped_project(
        project_dir=project_dir, files=test_case.files, stored_edits=test_case.stored_edits
    )
    for relative_path, old, new in test_case.edits:
        replace_project_text(project_dir, relative_path, old, new)
    copy: Path = tmp_path / "full_copy"
    _ = shutil.copytree(project_dir, copy, symlinks=True)

    edited: DeltaCompileOutcome = run_delta_compile(
        project_dir=project_dir, args=_TEXT_COMPILE, env={}
    )

    assert (DELTA_COMPILE_LINE in edited.stderr) == test_case.expected_delta_line, edited.stderr
    assert (
        full_compile_difference(
            outcome=edited,
            full=run_full_compile(copy, args=_TEXT_COMPILE),
            project_dir=project_dir,
            full_project_dir=copy,
        )
        == test_case.expected_difference
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaCompileVerifyTestCase(
            description="every shape together matches a full compile",
            files=(
                _CATEGORY_AMOUNTS,
                _CATEGORY_PIVOT,
                _ORDER_QUALITY_FACTORY,
                _MODEL_SQL_RULE,
                _MODEL_SQL_RULE_CASES,
            ),
            stored_edits=(_ATTACH_ORDER_QUALITY, _BUILT_IN_RULES_OFF),
            edits=(_BODY_EDIT,),
            env={DELTA_COMPILE_VERIFY_ENV_VAR: "1"},
            expected_stderr_line=DELTA_VERIFIED_LINE,
        ),
        DeltaCompileVerifyTestCase(
            description="an edit to dynamic column families names why it fell back",
            files=(_CATEGORY_AMOUNTS, _CATEGORY_PIVOT),
            stored_edits=(_BUILT_IN_RULES_OFF,),
            edits=(_FAMILY_TYPE_EDIT,),
            env={DELTA_COMPILE_VERIFY_ENV_VAR: "1"},
            expected_stderr_line=_FAMILIES_CHANGED,
        ),
        DeltaCompileVerifyTestCase(
            description="a rule listing models under a folder names why it fell back",
            files=(_FOLDER_RULE, _FOLDER_RULE_CASES),
            edits=(_BODY_EDIT,),
            env={DELTA_COMPILE_VERIFY_ENV_VAR: "1"},
            expected_stderr_line=_RULE_READ_MORE,
        ),
        DeltaCompileVerifyTestCase(
            description="a new tracked variable names what changed",
            files=(),
            edits=(_BODY_EDIT,),
            env={DELTA_COMPILE_VERIFY_ENV_VAR: "1", _EXAMPLE_TRACKED_VARIABLE: "1"},
            expected_stderr_line=_TRACKED_VARIABLE_ADDED,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_verified_edit_when_compiling_then_it_verifies_or_names_why_it_fell_back(
    test_case: DeltaCompileVerifyTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "dense_orders"
    write_delta_compile_project(project_dir=project_dir, model_count=_SMALL_MODEL_COUNT)
    store_shaped_project(
        project_dir=project_dir, files=test_case.files, stored_edits=test_case.stored_edits
    )
    for relative_path, old, new in test_case.edits:
        replace_project_text(project_dir, relative_path, old, new)

    edited: DeltaCompileOutcome = run_delta_compile(
        project_dir=project_dir, args=_TEXT_COMPILE, env=test_case.env
    )

    assert edited.returncode != MISMATCH_EXIT_CODE, edited.stderr[-2000:]
    assert test_case.expected_stderr_line in edited.stderr.splitlines(), edited.stderr[-2000:]


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaCompileSequenceTestCase(
            description="an unchanged project replays the delta",
            edits=(SequenceEdit.BODY, SequenceEdit.NONE),
            expected_delta_lines=(True, False),
            expected_replayed_last=True,
            expected_same_as_full=True,
        ),
        DeltaCompileSequenceTestCase(
            description="an edit after a multi-round delta is a delta too",
            edits=(SequenceEdit.SHAPE, SequenceEdit.BODY),
            expected_delta_lines=(True, True),
            expected_replayed_last=False,
            expected_same_as_full=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_edit_sequence_when_compiling_each_edit_then_deltas_store_and_replay_like_full_compiles(
    test_case: DeltaCompileSequenceTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "dense_orders"
    write_delta_compile_project(project_dir=project_dir, model_count=_MODEL_COUNT)

    sequence: DeltaCompileSequence = run_delta_compile_sequence(
        project_dir=project_dir,
        edits=test_case.edits,
        compile_project=partial(
            run_delta_compile, project_dir=project_dir, args=_JSON_COMPILE, env={}
        ),
        compile_in_full=partial(
            run_delta_compile,
            project_dir=project_dir,
            args=_JSON_COMPILE,
            env={DELTA_COMPILE_DISABLE_ENV_VAR: "1"},
        ),
    )

    assert sequence.delta_lines == test_case.expected_delta_lines
    assert sequence.replayed_last == test_case.expected_replayed_last
    assert (
        without_compile_timings(report=sequence.last_report)
        == without_compile_timings(report=sequence.full_report)
    ) == test_case.expected_same_as_full


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaCompileOptOutTestCase(
            description="project_setting", settings="\n[settings]\ndelta_compile = false\n", env={}
        ),
        DeltaCompileOptOutTestCase(
            description="environment", settings="", env={DELTA_COMPILE_DISABLE_ENV_VAR: "1"}
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_delta_compiles_turned_off_when_compiling_an_edit_then_it_compiles_in_full(
    test_case: DeltaCompileOptOutTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "dense_orders"
    write_delta_compile_project(project_dir=project_dir, model_count=_MODEL_COUNT)
    config: Path = project_dir / PROJECT_CONFIG
    config.write_text(config.read_text(encoding="utf-8") + test_case.settings, encoding="utf-8")
    first: DeltaCompileOutcome = run_delta_compile(
        project_dir=project_dir, args=_TEXT_COMPILE, env=test_case.env
    )
    replace_project_text(
        project_dir, _EDITED_MODEL, "COALESCE(b.amount, ", "COALESCE(b.amount, 1 + "
    )

    edited: DeltaCompileOutcome = run_delta_compile(
        project_dir=project_dir, args=_TEXT_COMPILE, env=test_case.env
    )

    assert (first.returncode, edited.returncode) == (0, 0), edited.stderr[-2000:]
    assert (DELTA_COMPILE_LINE in edited.stderr) == test_case.expected_delta_line
    assert (
        tuple(sorted(path.name for path in (project_dir / _REUSE_ROOT).glob("*.image")))
        == test_case.expected_images
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
