"""Real CLI coverage for targets that rely on the only named connection."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    DefaultConnectionCliTestCase,
    InferredFreshnessCliTestCase,
    LocalOverrideCliTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    execute_duckdb_sql,
    query_duckdb_rows,
    write_default_connection_project,
    write_default_connection_sql_test,
)

_ONE_CONNECTION_PROJECT: str = (
    'name = "shop"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connections.local]\n"
    'database = "shop.duckdb"\n\n'
    "[targets.dev]\n"
    'schema = "dev"\n'
)
_SEVERAL_CONNECTIONS_PROJECT: str = (
    'name = "shop"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connections.local]\n"
    'database = "shop.duckdb"\n\n'
    "[connections.archive]\n"
    'database = "archive.duckdb"\n\n'
    "[targets.dev]\n"
    'schema = "dev"\n'
)
_NO_CONNECTION_PROJECT: str = (
    'name = "shop"\nadapter = "duckdb"\ndefault_target = "dev"\n\n[targets.dev]\nschema = "dev"\n'
)
_TEMPLATED_CONNECTION_PROJECT: str = (
    'name = "shop"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[vars]\n"
    'db_name = "shop"\n\n'
    "[connections.local]\n"
    'database = "${db_name}.duckdb"\n\n'
    "[connections.archive]\n"
    'database = "archive.duckdb"\n\n'
    "[targets.dev]\n"
    'connection = "local"\n'
    'schema = "dev"\n'
)
_MEMORY_CONNECTION_PROJECT: str = (
    'name = "shop"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connections.memory]\n"
    'database = ":memory:"\n\n'
    "[targets.dev]\n"
    'schema = "main"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        DefaultConnectionCliTestCase(
            description="build writes into the only named connection",
            project_toml=_ONE_CONNECTION_PROJECT,
            argv=("build",),
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_order_rows=((1,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_without_connection_when_building_then_only_named_connection_is_used(
    test_case: DefaultConnectionCliTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_default_connection_project(project_dir=tmp_path, project_toml=test_case.project_toml)

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), *test_case.argv])

    output: str = "".join(capsys.readouterr())
    assert exit_code == test_case.expected_exit_code, output
    assert all(fragment in output for fragment in test_case.expected_output_fragments), output
    assert (
        query_duckdb_rows(db_path=tmp_path / "shop.duckdb", sql="SELECT order_id FROM dev.orders")
        == test_case.expected_order_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LocalOverrideCliTestCase(
            description="local vars rename the templated database and local schema wins",
            local_toml='[vars]\ndb_name = "mine"\n\n[targets.dev]\nschema = "staging"\n',
            argv=("build",),
            expected_exit_code=0,
            expected_database="mine.duckdb",
            expected_schema="staging",
            expected_order_rows=((1,),),
        ),
        LocalOverrideCliTestCase(
            description="CLI vars beat local vars and a local target switches connection",
            local_toml='[vars]\ndb_name = "mine"\n\n[targets.dev]\nconnection = "archive"\n',
            argv=("build", "--vars", '{"db_name": "cli"}'),
            expected_exit_code=0,
            expected_database="archive.duckdb",
            expected_schema="dev",
            expected_order_rows=((1,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_local_and_cli_overrides_when_building_then_precedence_picks_the_relation(
    test_case: LocalOverrideCliTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_default_connection_project(
        project_dir=tmp_path, project_toml=_TEMPLATED_CONNECTION_PROJECT
    )
    _ = (tmp_path / "sqlbuild_local.toml").write_text(test_case.local_toml, encoding="utf-8")

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), *test_case.argv])

    output: str = "".join(capsys.readouterr())
    assert exit_code == test_case.expected_exit_code, output
    assert (
        query_duckdb_rows(
            db_path=tmp_path / test_case.expected_database,
            sql=f"SELECT order_id FROM {test_case.expected_schema}.orders",
        )
        == test_case.expected_order_rows
    )
    assert not (tmp_path / "shop.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        DefaultConnectionCliTestCase(
            description="debug reports the only named connection",
            project_toml=_ONE_CONNECTION_PROJECT,
            argv=("debug", "--no-connection"),
            expected_exit_code=0,
            expected_output_fragments=("connection: local", "database: not configured"),
        ),
        DefaultConnectionCliTestCase(
            description="build fails at config load among several named connections",
            project_toml=_SEVERAL_CONNECTIONS_PROJECT,
            argv=("build",),
            expected_exit_code=1,
            expected_output_fragments=(
                "error[D001]",
                "targets.dev does not set connection and several named connections are "
                "defined (archive, local)",
            ),
        ),
        DefaultConnectionCliTestCase(
            description="build fails before connecting without any connection",
            project_toml=_NO_CONNECTION_PROJECT,
            argv=("build",),
            expected_exit_code=1,
            expected_output_fragments=(
                "error[D001]",
                "target dev has no connection; add [connections.<name>] (or set connection = "
                '"<name>" on [targets.dev]) in sqlbuild_project.toml or sqlbuild_local.toml',
            ),
        ),
        DefaultConnectionCliTestCase(
            description="compile works without any connection",
            project_toml=_NO_CONNECTION_PROJECT,
            argv=("compile",),
            expected_exit_code=0,
            expected_output_fragments=("Project compiled",),
        ),
        DefaultConnectionCliTestCase(
            description="freshness without selected sources works without any connection",
            project_toml=_NO_CONNECTION_PROJECT,
            argv=("freshness",),
            expected_exit_code=0,
            expected_output_fragments=("OBSERVED=0",),
        ),
        DefaultConnectionCliTestCase(
            description="build uses an explicit in-memory connection",
            project_toml=_MEMORY_CONNECTION_PROJECT,
            argv=("build",),
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_without_connection_when_resolving_offline_then_connection_is_reported(
    test_case: DefaultConnectionCliTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_default_connection_project(project_dir=tmp_path, project_toml=test_case.project_toml)

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), *test_case.argv])

    output: str = "".join(capsys.readouterr())
    assert exit_code == test_case.expected_exit_code, output
    assert all(fragment in output for fragment in test_case.expected_output_fragments), output
    assert not (tmp_path / "shop.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        DefaultConnectionCliTestCase(
            description="test inspection works without any connection",
            project_toml=_NO_CONNECTION_PROJECT,
            argv=("test", "--inspect"),
            expected_exit_code=0,
            expected_output_fragments=(
                "boundary: orders is replaced by __ref__orders",
                "Test plan inspection complete: 1 selected, 0 errors.",
            ),
        ),
        DefaultConnectionCliTestCase(
            description="test execution fails before connecting without any connection",
            project_toml=_NO_CONNECTION_PROJECT,
            argv=("test",),
            expected_exit_code=1,
            expected_output_fragments=("error[D001]", "target dev has no connection"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_connection_when_running_sql_tests_then_only_inspection_succeeds(
    test_case: DefaultConnectionCliTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_default_connection_project(project_dir=tmp_path, project_toml=test_case.project_toml)
    write_default_connection_sql_test(project_dir=tmp_path)

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), *test_case.argv])

    output: str = "".join(capsys.readouterr())
    assert exit_code == test_case.expected_exit_code, output
    assert all(fragment in output for fragment in test_case.expected_output_fragments), output
    assert "Connecting to" not in output
    assert not (tmp_path / "shop.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        InferredFreshnessCliTestCase(
            description="column key and declared timestamp column imply column timestamp",
            sources_yaml=(
                "sources:\n"
                "  - name: raw_orders\n"
                "    description: Raw orders.\n"
                "    schema: raw\n"
                "    table: orders\n"
                "    columns:\n"
                "      - name: updated_at\n"
                "        type: timestamp\n"
                "    freshness:\n"
                "      column: updated_at\n"
            ),
            expected_sources=(("raw_orders", "column", "timestamp", "2026-01-02T03:04:05+00:00"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_freshness_without_strategy_or_type_when_observing_then_both_are_inferred(
    test_case: InferredFreshnessCliTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_default_connection_project(project_dir=tmp_path, project_toml=_ONE_CONNECTION_PROJECT)
    (tmp_path / "sources").mkdir()
    _ = (tmp_path / "sources" / "raw.yml").write_text(test_case.sources_yaml, encoding="utf-8")
    execute_duckdb_sql(
        db_path=tmp_path / "shop.duckdb",
        sql=(
            "CREATE SCHEMA raw; "
            "CREATE TABLE raw.orders AS SELECT TIMESTAMP '2026-01-02 03:04:05' AS updated_at"
        ),
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "freshness", "--json"])

    captured: str = capsys.readouterr().out
    assert exit_code == 0, captured
    sources: list[dict[str, object]] = json.loads(captured)["sources"]
    assert (
        tuple(
            (
                source["name"],
                source["strategy"],
                source["value_kind"],
                source["current_data_version"],
            )
            for source in sources
        )
        == test_case.expected_sources
    )
