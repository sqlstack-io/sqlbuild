"""E2E tests for managed resource namespace validation during compile."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    NamespaceCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PRESERVE_CONFIG: str = (
    dedent(
        """
    name = "namespace_validation"
    adapter = "duckdb"
    default_target = "dev"

    [connection]
    database = ":memory:"

    [targets.dev]
    schema = "preserve"
    """
    ).strip()
    + "\n"
)
_SEED_SCHEMA: str = (
    dedent(
        """
    seeds:
      - name: countries
        description: Test seed countries.
        columns:
          - name: code
            type: VARCHAR
    """
    ).strip()
    + "\n"
)
_POSTGRES_CONFIG_WITHOUT_SCHEMA: str = (
    'name = "namespace_validation"\nadapter = "postgres"\n\n[connection]\ndatabase = "analytics"\n'
)
_NAMED_POSTGRES_CONFIG_WITHOUT_SCHEMA: str = (
    'name = "namespace_validation"\nadapter = "postgres"\ndefault_target = "dev"\n\n'
    '[connection]\ndatabase = "analytics"\n\n[targets.dev]\n'
)
_PYTHON_FUNCTION: str = (
    "from sqlbuild.functions import udf\n\n"
    '@udf(arguments={}, returns="INTEGER", runtime_version="3.11")\n'
    "def main():\n"
    "    '''Test function main.'''\n    return 42\n"
)
_MANAGED_SOURCE: str = (
    "sources:\n"
    "  - name: raw_events\n    description: Test source raw_events.\n"
    "    managed: true\n"
    "    write_strategy: table\n"
    "    columns:\n"
    "      - name: event_id\n"
    "        type: INTEGER\n"
)
_MANAGED_SOURCE_LOADER: str = (
    "from sqlbuild.loaders import loader\n\n"
    "@loader\n"
    "def raw_events(ctx):\n"
    "    '''Test loader raw_events.'''\n    return [{'event_id': 1}]\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        NamespaceCompileTestCase(
            description="model without logical schema fails under preserve",
            repo_files={
                "sqlbuild_project.toml": _PRESERVE_CONFIG,
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Model 'orders' has no logical schema",
        ),
        NamespaceCompileTestCase(
            description="seed without logical schema fails under preserve",
            repo_files={
                "sqlbuild_project.toml": _PRESERVE_CONFIG,
                "sql/seeds/schema.yml": _SEED_SCHEMA,
                "sql/seeds/countries.csv": "code\nGB\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Seed 'countries' has no logical schema",
        ),
        NamespaceCompileTestCase(
            description="SQL scalar function without logical schema fails under preserve",
            repo_files={
                "sqlbuild_project.toml": _PRESERVE_CONFIG,
                "sql/functions/sql/answer.sql": "FUNCTION (description 'Test function answer.', returns INTEGER);\n\n42\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="SQL function 'answer' has no logical schema",
        ),
        NamespaceCompileTestCase(
            description="SQL table function without logical schema fails under preserve",
            repo_files={
                "sqlbuild_project.toml": _PRESERVE_CONFIG,
                "sql/functions/sql/answers.sql": dedent(
                    """
                FUNCTION (description "Test function answers.",
                  returns table (
                    answer INTEGER
                  )
                );

                SELECT 42 AS answer
                """
                ).strip()
                + "\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="SQL function 'answers' has no logical schema",
        ),
        NamespaceCompileTestCase(
            description="non inheriting Python UDF without logical schema fails under preserve",
            repo_files={
                "sqlbuild_project.toml": _PRESERVE_CONFIG,
                "sql/functions/python/answer.py": dedent(
                    """
                from sqlbuild.functions import udf

                @udf(arguments={}, returns="INTEGER", runtime_version="3.11")
                def main():
                    '''Test function main.'''
                    return 42
                """
                ).strip()
                + "\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Python function 'answer' has no logical schema",
        ),
        NamespaceCompileTestCase(
            description="managed source without logical schema fails under preserve",
            repo_files={
                "sqlbuild_project.toml": _PRESERVE_CONFIG,
                "sql/sources/raw.yml": _MANAGED_SOURCE,
                "python/nodes/loaders/raw_events/loader.py": _MANAGED_SOURCE_LOADER,
            },
            expected_exit_code=1,
            expected_stderr_fragment="Managed source 'raw_events' has no logical schema",
        ),
        NamespaceCompileTestCase(
            description="resource specific defaults resolve preserved schemas",
            repo_files={
                "sqlbuild_project.toml": dedent(
                    """
                name = "namespace_validation"
                adapter = "duckdb"
                default_target = "dev"

                [connection]
                database = ":memory:"

                [defaults]
                seed_schema = "seed_data"
                function_schema = "functions"

                [path_defaults.marts]
                schema = "analytics"

                [targets.dev]
                schema = "preserve"
                """
                ).strip()
                + "\n",
                "sql/models/marts/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
                "sql/seeds/schema.yml": _SEED_SCHEMA,
                "sql/seeds/countries.csv": "code\nGB\n",
                "sql/functions/sql/answer.sql": "FUNCTION (description 'Test function answer.', returns INTEGER);\n\n42\n",
                "sql/functions/python/python_answer.py": dedent(
                    """
                from sqlbuild.functions import udf

                @udf(arguments={}, returns="INTEGER", runtime_version="3.11")
                def main():
                    '''Test function main.'''
                    return 42
                """
                ).strip()
                + "\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="literal target schema supplies physical namespace",
            repo_files={
                "sqlbuild_project.toml": _PRESERVE_CONFIG.replace(
                    'schema = "preserve"', 'schema = "physical"'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
                "sql/seeds/schema.yml": _SEED_SCHEMA,
                "sql/seeds/countries.csv": "code\nGB\n",
                "sql/functions/sql/answer.sql": "FUNCTION (description 'Test function answer.', returns INTEGER);\n\n42\n",
                "sql/functions/python/python_answer.py": dedent(
                    """
                from sqlbuild.functions import udf

                @udf(arguments={}, returns="INTEGER", runtime_version="3.11")
                def main():
                    '''Test function main.'''
                    return 42
                """
                ).strip()
                + "\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="warehouse model without explicit schema fails offline",
            repo_files={
                "sqlbuild_project.toml": _POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Model 'orders' has no explicitly resolved physical write schema",
        ),
        NamespaceCompileTestCase(
            description="warehouse seed without explicit schema fails offline",
            repo_files={
                "sqlbuild_project.toml": _POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/seeds/schema.yml": _SEED_SCHEMA,
                "sql/seeds/countries.csv": "code\nGB\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Seed 'countries' has no explicitly resolved physical write schema",
        ),
        NamespaceCompileTestCase(
            description="warehouse SQL scalar function without explicit schema fails offline",
            repo_files={
                "sqlbuild_project.toml": _POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/functions/sql/answer.sql": "FUNCTION (description 'Test function answer.', returns INTEGER);\n\n42\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Function 'answer' has no explicitly resolved physical write schema",
        ),
        NamespaceCompileTestCase(
            description="warehouse SQL table function without explicit schema fails offline",
            repo_files={
                "sqlbuild_project.toml": _POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/functions/sql/answers.sql": (
                    "FUNCTION (description 'Test function answers.', returns table (answer INTEGER));\n\nSELECT 42 AS answer\n"
                ),
            },
            expected_exit_code=1,
            expected_stderr_fragment="Function 'answers' has no explicitly resolved physical write schema",
        ),
        NamespaceCompileTestCase(
            description="warehouse Python UDF without explicit schema fails offline",
            repo_files={
                "sqlbuild_project.toml": _POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/functions/python/answer.py": _PYTHON_FUNCTION,
            },
            expected_exit_code=1,
            expected_stderr_fragment="Function 'answer' has no explicitly resolved physical write schema",
        ),
        NamespaceCompileTestCase(
            description="warehouse managed source without explicit schema fails offline",
            repo_files={
                "sqlbuild_project.toml": _POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/sources/raw.yml": _MANAGED_SOURCE,
                "python/nodes/loaders/raw_events/loader.py": _MANAGED_SOURCE_LOADER,
            },
            expected_exit_code=1,
            expected_stderr_fragment=(
                "Managed source 'raw_events' has no explicitly resolved physical write schema"
            ),
        ),
        NamespaceCompileTestCase(
            description="resource schema supplies physical namespace",
            repo_files={
                "sqlbuild_project.toml": _POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/models/orders.sql": "MODEL (description 'Test model orders.', schema analytics);\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="default schema supplies physical namespace",
            repo_files={
                "sqlbuild_project.toml": (
                    _POSTGRES_CONFIG_WITHOUT_SCHEMA + '\n[defaults]\nschema = "analytics"\n'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="literal target schema supplies warehouse physical namespace",
            repo_files={
                "sqlbuild_project.toml": (
                    _POSTGRES_CONFIG_WITHOUT_SCHEMA.replace(
                        "\n[connection]\n", '\ndefault_target = "dev"\n\n[connection]\n'
                    )
                    + '\n[targets.dev]\nschema = "analytics"\n'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="preserve target schema retains model logical namespace",
            repo_files={
                "sqlbuild_project.toml": (
                    _NAMED_POSTGRES_CONFIG_WITHOUT_SCHEMA + 'schema = "preserve"\n'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.', schema analytics);\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="named target rejects model logical schema as strategy",
            repo_files={
                "sqlbuild_project.toml": _NAMED_POSTGRES_CONFIG_WITHOUT_SCHEMA,
                "sql/models/orders.sql": "MODEL (description 'Test model orders.', schema analytics);\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment=(
                "Named target 'dev' must explicitly set schema to a nonblank literal or 'preserve'"
            ),
        ),
        NamespaceCompileTestCase(
            description="named target rejects defaults schema as strategy",
            repo_files={
                "sqlbuild_project.toml": (
                    _NAMED_POSTGRES_CONFIG_WITHOUT_SCHEMA + '\n[defaults]\nschema = "analytics"\n'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Named target 'dev' must explicitly set schema",
        ),
        NamespaceCompileTestCase(
            description="named target rejects connection schema as strategy",
            repo_files={
                "sqlbuild_project.toml": _NAMED_POSTGRES_CONFIG_WITHOUT_SCHEMA.replace(
                    'database = "analytics"',
                    'database = "analytics"\nschema = "connection_schema"',
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=1,
            expected_stderr_fragment="Named target 'dev' must explicitly set schema",
        ),
        NamespaceCompileTestCase(
            description="connection schema supplies physical namespace",
            repo_files={
                "sqlbuild_project.toml": (
                    _POSTGRES_CONFIG_WITHOUT_SCHEMA + 'schema = "analytics"\n'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="named target rejects loader schema as target strategy",
            repo_files={
                "sqlbuild_project.toml": (
                    _POSTGRES_CONFIG_WITHOUT_SCHEMA.replace(
                        "\n[connection]\n", '\ndefault_target = "dev"\n\n[connection]\n'
                    )
                    + '\n[targets.dev]\nloader_schema = "raw"\n'
                ),
                "sql/sources/raw.yml": _MANAGED_SOURCE,
                "python/nodes/loaders/raw_events/loader.py": _MANAGED_SOURCE_LOADER,
            },
            expected_exit_code=1,
            expected_stderr_fragment="Named target 'dev' must explicitly set schema",
        ),
        NamespaceCompileTestCase(
            description="DuckDB may compile managed writes with implicit main schema",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "namespace_validation"\nadapter = "duckdb"\n\n'
                    "[connection]\n"
                    'database = ":memory:"\n'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
                "sql/seeds/schema.yml": _SEED_SCHEMA,
                "sql/seeds/countries.csv": "code\nGB\n",
                "sql/functions/sql/answer.sql": "FUNCTION (description 'Test function answer.', returns INTEGER);\n\n42\n",
                "sql/sources/raw.yml": _MANAGED_SOURCE,
                "python/nodes/loaders/raw_events/loader.py": _MANAGED_SOURCE_LOADER,
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
        NamespaceCompileTestCase(
            description="named DuckDB target may compile with implicit main schema",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "namespace_validation"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                    "[connection]\n"
                    'database = ":memory:"\n\n'
                    "[targets.dev]\n"
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            expected_exit_code=0,
            expected_stderr_fragment="",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_managed_resources_when_compiling_namespace_targets_then_validation_is_fail_closed(
    test_case: NamespaceCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="namespace_validation",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr
