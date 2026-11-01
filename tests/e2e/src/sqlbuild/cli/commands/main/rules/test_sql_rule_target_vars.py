"""E2E coverage of SQL Rules linting the SQL the command's `--target` and `--vars` expand."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import SqlRuleTargetVarsCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
    '[connection]\ndatabase = "warehouse.duckdb"\n\n'
    '[vars]\nprobe = "1"\n\n[targets.dev]\n\n[targets.prod]\n\n'
    '[targets.prod.vars]\nprobe = "NULL"\nregion = "us"\n'
)
_MODEL_PATH: str = "models/orders.sql"
_MODEL_SQL: str = (
    "MODEL (description 'Test model orders.');\n"
    'SELECT o.id FROM __ref("raw_orders") AS o WHERE o.id = @@probe\n'
)
_UPSTREAM_FILES: dict[str, str] = {
    "models/raw_orders.sql": "MODEL (description 'Test model raw orders.');\nSELECT 1 AS id\n"
}
_PROD_ONLY_MODEL: dict[str, str] = {
    "models/regions.sql": (
        "MODEL (description 'Test model regions.');\nSELECT '@@region' AS region\n"
    )
}
_AUDIT_PATH: str = "audits/singular/orders_have_ids.sql"
_AUDIT_BODY: str = (
    'SELECT o.id\nFROM __ref("orders") AS o\n'
    'INNER JOIN __ref("raw_orders") AS r ON o.id = r.id\nWHERE o.id = NULL\n'
)
_TEST_PATH: str = "tests/unit/test_orders.sql"
_TEST_BODY: str = (
    "WITH __ref__raw_orders AS (SELECT 1 AS id),\n"
    "__expected__orders AS (SELECT 1 AS id WHERE 1 = NULL)\nSELECT 1\n"
)
_NULL_FINDING: str = "SQLCLEAN001"
_UNKNOWN_VAR: str = "L005"
_COMPILE: tuple[str, ...] = ("compile",)
_PROD: tuple[str, ...] = ("compile", "--target", "prod")
_NULL_VARS: tuple[str, ...] = ("compile", "--vars", '{"probe": "NULL"}')
_CLEAN: tuple[int, bool, bool] = (0, False, False)
_NULL: tuple[int, bool, bool] = (1, True, False)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlRuleTargetVarsCase(
            description="default target lints its own vars",
            commands=(_COMPILE,),
            expected_outcomes=(_CLEAN,),
        ),
        SqlRuleTargetVarsCase(
            description="--target lints that target's vars cold and warm",
            commands=(_PROD, _PROD),
            expected_outcomes=(_NULL, _NULL),
        ),
        SqlRuleTargetVarsCase(
            description="--target without the cache lints that target's vars",
            commands=(("compile", "--no-cache", "--target", "prod"),),
            expected_outcomes=(_NULL,),
        ),
        SqlRuleTargetVarsCase(
            description="--vars overrides the default target's vars cold and warm",
            commands=(_NULL_VARS, _NULL_VARS),
            expected_outcomes=(_NULL, _NULL),
        ),
        SqlRuleTargetVarsCase(
            description="--vars after a default compile reuses no findings from it",
            commands=(_COMPILE, _NULL_VARS),
            expected_outcomes=(_CLEAN, _NULL),
        ),
        SqlRuleTargetVarsCase(
            description="switching targets reuses no findings from the other target",
            commands=(_COMPILE, _PROD, _COMPILE),
            expected_outcomes=(_CLEAN, _NULL, _CLEAN),
        ),
        SqlRuleTargetVarsCase(
            description="selected model lints with --target",
            commands=(("compile", "--target", "prod", "--select", "orders"),),
            expected_outcomes=(_NULL,),
        ),
        SqlRuleTargetVarsCase(
            description="plan lints with --target",
            commands=(("plan", "--target", "prod"),),
            expected_outcomes=(_NULL,),
        ),
        SqlRuleTargetVarsCase(
            description="build lints with --target",
            commands=(("build", "--target", "prod"),),
            expected_outcomes=(_NULL,),
        ),
        SqlRuleTargetVarsCase(
            description="a var only the selected target defines expands without L005",
            commands=(("compile", "--target", "prod", "--vars", '{"probe": "1"}'),),
            expected_outcomes=(_CLEAN,),
            extra_files=_PROD_ONLY_MODEL,
        ),
        SqlRuleTargetVarsCase(
            description="--target still lints audit and SQL test files without an opt-out",
            commands=(("compile", "--target", "prod", "--vars", '{"probe": "1"}'),),
            expected_outcomes=(_NULL,),
            extra_files={
                _AUDIT_PATH: "AUDIT ();\n" + _AUDIT_BODY,
                _TEST_PATH: "TEST ();\n" + _TEST_BODY,
            },
        ),
        SqlRuleTargetVarsCase(
            description="--target keeps audit and SQL test sql_analysis opt-outs",
            commands=(("compile", "--target", "prod", "--vars", '{"probe": "1"}'),),
            expected_outcomes=(_CLEAN,),
            extra_files={
                _AUDIT_PATH: "AUDIT (sql_analysis false);\n" + _AUDIT_BODY,
                _TEST_PATH: "TEST (sql_analysis false);\n" + _TEST_BODY,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_and_vars_when_compiling_then_sql_rules_lint_that_expansion(
    test_case: SqlRuleTargetVarsCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            _MODEL_PATH: _MODEL_SQL,
            **_UPSTREAM_FILES,
            **test_case.extra_files,
        },
    )

    outcomes: list[tuple[int, bool, bool]] = []
    outputs: list[str] = []
    for command in test_case.commands:
        ran: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", *command), project_dir=project_dir
        )
        output: str = ran.stdout + ran.stderr
        outputs.append(output)
        outcomes.append((ran.returncode, _NULL_FINDING in output, _UNKNOWN_VAR in output))

    assert tuple(outcomes) == test_case.expected_outcomes, "\n----\n".join(outputs)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
