"""A missing secret's advice names the env file in use, and the flag only where a command takes it."""

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SecretAdviceTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    REGION_PROVIDER_FILES,
    run_sqb,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_SET_HERE: str = "set WORKER_ENV_TOKEN, WORKER_ENV_REGION_CODE in this shell or in "


@pytest.mark.parametrize(
    "test_case",
    [
        SecretAdviceTestCase("compile takes the flag", ("compile",), ".env", _SET_HERE, True),
        SecretAdviceTestCase("dag takes the flag", ("dag",), ".env", _SET_HERE, True),
        SecretAdviceTestCase(
            "lineage takes the flag", ("lineage", "orders"), ".env", _SET_HERE, True
        ),
        SecretAdviceTestCase(
            "a subcommand takes the flag", ("rules", "run", "all"), ".env", _SET_HERE, True
        ),
        SecretAdviceTestCase("plan doesn't", ("plan",), ".env", _SET_HERE, False),
        SecretAdviceTestCase("freshness doesn't", ("freshness",), ".env", _SET_HERE, False),
        SecretAdviceTestCase("janitor doesn't", ("janitor",), ".env", _SET_HERE, False),
        SecretAdviceTestCase(
            "a subcommand doesn't", ("scenario", "test"), ".env", _SET_HERE, False
        ),
        SecretAdviceTestCase(
            "compile names the target's own env file",
            ("compile",),
            "development.env",
            _SET_HERE,
            True,
        ),
        SecretAdviceTestCase(
            "build names the target's own env file",
            ("build",),
            "development.env",
            "set each variable in this shell or in ",
            False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_missing_secret_when_a_command_fails_then_the_help_fits_that_command(
    test_case: SecretAdviceTestCase, tmp_path: Path
) -> None:
    write_files(
        project_dir=tmp_path,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "development"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
                f'env_file = "{test_case.env_file_name}"\n'
            ),
            "models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n"
                'SELECT CAST(code AS BIGINT) AS amount FROM __source("raw_regions")\n'
            ),
        }
        | REGION_PROVIDER_FILES,
    )
    env_file: Path = tmp_path / test_case.env_file_name
    _ = env_file.write_text("UNRELATED_SETTING=1\n")
    env_file.chmod(0o600)

    result = run_sqb(tmp_path, "--no-color", *test_case.command)
    shown: str = " ".join((result.stdout + result.stderr).split())

    assert result.returncode != 0, shown
    assert test_case.expected_help + test_case.env_file_name in shown, shown
    assert ("--allow-missing-secrets" in shown) is test_case.expected_flag_suggested, shown
