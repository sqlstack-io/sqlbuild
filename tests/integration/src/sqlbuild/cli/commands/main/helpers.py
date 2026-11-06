from __future__ import annotations

import _thread
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from collections.abc import Callable
from itertools import takewhile
from pathlib import Path
from types import ModuleType
from typing import Any, cast

import duckdb
import pytest

import sqlbuild._native as native_module
from sqlbuild._native import (
    CompilerStage,
    CustomHostObserver,
    PreparedCompileArtifacts,
)
from sqlbuild._native import run_cli as main
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    RenderedRetentionChange,
    RetentionRequest,
    RetentionState,
)
from sqlbuild.adapter.contract.types import RetentionChangePhase
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    RepeatedJsonParseTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.run.helpers import rebuild_identity

STAGE_CAPTURE_SUFFIX: str = ".json"

E2E_FIXTURES_DIR: Path = Path(__file__).resolve().parents[6] / "e2e" / "fixtures"
_WEEKLY_ORDERS: str = (
    'MODEL (\n  description "Weekly orders per product",\n  materialized table,\n);\n\n'
    "SELECT 1 AS product_id, CAST(10 AS BIGINT) AS orders_placed, "
    "CAST(1 AS BIGINT) AS refund_count\n"
)
_REFUND_RATES: str = (
    'MODEL (\n  description "Weekly refund rate per product",\n  materialized table,\n);\n\n'
    'WITH weekly AS (SELECT * FROM __ref("weekly_orders")),\n'
    "@sql.weekly_rates(weekly, weekly.keys := (product_id), "
    "weekly.order_count := orders_placed, __cte := by_product)\n"
    "final AS (SELECT * FROM by_product)\n"
    "SELECT * FROM final\n"
)


def artifact_tree(root: Path) -> dict[str, bytes]:
    """Return each file under root by its POSIX path relative to root, or nothing when missing."""

    files: list[Path] = sorted(filter(Path.is_file, root.rglob("*")))
    return {path.relative_to(root).as_posix(): path.read_bytes() for path in files}


def changed_artifacts(before: dict[str, bytes], after: dict[str, bytes]) -> list[str]:
    """Return the sorted paths whose bytes differ between two artifact trees, or exist in one."""

    return sorted(filter(lambda path: before.get(path) != after.get(path), before | after))


def staging_directories(target_dir: Path) -> list[str]:
    """Return hidden entries left in target_dir, such as abandoned artifact staging."""

    return sorted(path.name for path in target_dir.glob(".*"))


def write_staging_directory(*, target_dir: Path, name: str) -> Path:
    """Write one artifact staging directory with a staged model file."""

    directory: Path = target_dir / name
    (directory / "compiled" / "models").mkdir(parents=True)
    (directory / "compiled" / "models" / "orders.sql").write_text("SELECT 1\n", encoding="utf-8")
    return directory


def write_prepared_artifacts_project(*, project_dir: Path, model_count: int) -> None:
    """Write a DuckDB project large enough to stage compile artifacts in the background."""

    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = ["SQLCLEAN021"]\n',
        encoding="utf-8",
    )
    models: Path = project_dir / "models"
    models.mkdir()
    for index in range(model_count):
        (models / f"orders_{index:03}.sql").write_text(
            "MODEL (description 'Test model.', materialized table, contract enforced, "
            "columns (order_id (type INTEGER)));\n"
            f"SELECT CAST({index} AS INTEGER) AS order_id\n",
            encoding="utf-8",
        )


SELECTED_RULES_CONFIG: str = '[rules]\nselect = ["SQLCLEAN021"]\n'
NO_RULES_CONFIG: str = "[rules]\nselect = []\n"
TEST_PLANNING_MODELS: int = 6


def write_test_planning_project(
    *, project_dir: Path, model_count: int, rules_config: str, test_options: str = ""
) -> None:
    """Write a chain of DuckDB models with one SQL test per downstream model."""

    project_dir.mkdir(parents=True, exist_ok=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "duckdb"\n{rules_config}', encoding="utf-8"
    )
    models: Path = project_dir / "models"
    tests: Path = project_dir / "tests" / "unit"
    models.mkdir()
    tests.mkdir(parents=True)
    header: str = (
        "MODEL (description 'Test model.', materialized table, contract enforced, "
        "columns (order_id (type INTEGER)));\n"
    )
    (models / "orders_000.sql").write_text(
        header + "SELECT CAST(0 AS INTEGER) AS order_id\n", encoding="utf-8"
    )
    for index in range(1, model_count):
        upstream: str = f"orders_{index - 1:03}"
        (models / f"orders_{index:03}.sql").write_text(
            header + f'SELECT CAST(order_id + 1 AS INTEGER) AS order_id FROM __ref("{upstream}")\n',
            encoding="utf-8",
        )
        write_order_test(
            tests_dir=tests, index=index, expected_order_id=2, test_options=test_options
        )


def write_order_test(
    *, tests_dir: Path, index: int, expected_order_id: int, test_options: str = ""
) -> None:
    """Write the SQL test for one chained order model with a mocked upstream model."""

    (tests_dir / f"orders_{index:03}.sql").write_text(
        f'TEST (name "orders_{index:03}_case"{test_options});\n'
        "WITH\n"
        f"__ref__orders_{index - 1:03} AS (SELECT CAST(1 AS INTEGER) AS order_id),\n"
        f"__expected__orders_{index:03} AS "
        f"(SELECT CAST({expected_order_id} AS INTEGER) AS order_id)\n"
        "SELECT 1\n",
        encoding="utf-8",
    )


def compiled_files(project_dir: Path) -> dict[str, bytes]:
    """Return every compiled artifact under target/compiled keyed by relative path."""

    compiled: Path = project_dir / "target" / "compiled"
    return {
        str(path.relative_to(compiled)): path.read_bytes()
        for path in sorted(compiled.rglob("*.sql"))
    }


def semantic_compile_payload(output: str) -> dict[str, object]:
    """Return compile JSON without its timings and version, which vary between runs."""

    payload: dict[str, object] = json.loads(output)
    payload.pop("compile_timings", None)
    payload.pop("version", None)
    return payload


def compile_project_json(
    *, project_dir: Path, compile_args: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> tuple[int, dict[str, object]]:
    """Compile through the CLI and return the exit code with the semantic JSON payload."""

    exit_code: int = main(["--project-dir", str(project_dir), "compile", "--json", *compile_args])
    return exit_code, semantic_compile_payload(capsys.readouterr().out)


def keep_project_unchanged(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Leave a freshly written project cold, with no target directory."""


def compile_then_edit_one_test(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Warm the compile cache, then change one SQL test so only it must be planned again."""

    assert compile_project_json(project_dir=project_dir, compile_args=(), capsys=capsys)[0] == 0
    write_order_test(tests_dir=project_dir / "tests" / "unit", index=2, expected_order_id=3)


def keep_native_planner(monkeypatch: pytest.MonkeyPatch) -> None:
    """Leave the real native SQL-test planner in place."""


def use_prepared_artifacts(
    monkeypatch: pytest.MonkeyPatch, *, model_range: tuple[int, int]
) -> None:
    """Compile with background preparation staging projects of this model range."""

    class Prepared(PreparedCompileArtifacts):
        min_prepared_models: int = model_range[0]
        max_prepared_models: int = model_range[1]

    monkeypatch.setattr(native_module, "PreparedCompileArtifacts", Prepared)


def use_staging_directory_factory(
    monkeypatch: pytest.MonkeyPatch, *, create_staging: Callable[[Path], None]
) -> None:
    """Compile with background staging that creates its directory through create_staging."""

    class Prepared(PreparedCompileArtifacts):
        def create_staging_directory(self, staging_dir: Path) -> None:
            create_staging(staging_dir)

    monkeypatch.setattr(native_module, "PreparedCompileArtifacts", Prepared)


def skip_artifact_preparation(monkeypatch: pytest.MonkeyPatch) -> None:
    """Disable background preparation so the write phase plans SQL tests inline."""

    class Prepared(PreparedCompileArtifacts):
        def start(self, **kwargs: Any) -> None:
            """Prepare nothing."""

    monkeypatch.setattr(native_module, "PreparedCompileArtifacts", Prepared)


def create_owned_staging_directory(staging_dir: Path) -> None:
    """Create a staging directory the way a compile does."""

    PreparedCompileArtifacts(enabled=True).create_staging_directory(staging_dir)


def has_second_filesystem(root: Path) -> bool:
    """Return whether root is a writable directory on another device than the temp directory."""

    return (
        root.is_dir()
        and os.access(root, os.W_OK)
        and root.stat().st_dev != Path(tempfile.gettempdir()).stat().st_dev
    )


def unavailable_staging_directory(staging_dir: Path) -> None:
    raise OSError(f"staging storage unavailable for {staging_dir.name}")


def write_compile_startup_project(project_dir: Path) -> None:
    """Write the minimal project used by fresh-process import tests."""

    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    models_dir: Path = project_dir / "models"
    models_dir.mkdir()
    (models_dir / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized table);\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )


def write_from_values_format_project(*, tmp_path: Path, adapter: str) -> tuple[Path, Path]:
    """Write a format project containing an authored values relation."""

    project_dir: Path = tmp_path / adapter
    project_dir.mkdir()
    (project_dir / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{adapter}"\n\n[rules]\nselect = ["SQLCLEAN039"]\n',
        encoding="utf-8",
    )
    models: Path = project_dir / "models"
    models.mkdir()
    (models / "customers.sql").write_text(
        'MODEL (description "Customers.");\nSELECT 1 AS customer_key, 1 AS order_count\n',
        encoding="utf-8",
    )
    (models / "orders.sql").write_text(
        'MODEL (description "Orders.");\nSELECT customer_key, 1 AS order_count '
        'FROM __ref("customers")\n',
        encoding="utf-8",
    )
    test_file: Path = project_dir / "tests" / "unit" / "test_orders.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_text(
        'TEST (name "orders_from_values");\n\n'
        "WITH __ref__customers AS (\n"
        "    SELECT COLUMN1::VARCHAR AS customer_key, COLUMN2::INTEGER AS order_count\n"
        "    FROM VALUES\n"
        "        ('c1', 1),\n"
        "        ('c2', 2)\n"
        "),\n"
        "__expected__orders AS (SELECT 'c1' AS customer_key, 1 AS order_count)\n"
        "SELECT 1\n",
        encoding="utf-8",
    )
    return project_dir, test_file


def write_snowflake_format_test(*, tmp_path: Path, test_sql: str) -> tuple[Path, Path]:
    """Write a minimal Snowflake-dialect project for real CLI formatting."""

    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "products"\nadapter = "snowflake"\n', encoding="utf-8"
    )
    test_file: Path = tmp_path / "tests" / "unit" / "test_products.sql"
    test_file.parent.mkdir(parents=True)
    test_file.write_text(test_sql, encoding="utf-8")
    return tmp_path, test_file


def prepare_contract_project(tmp_path: Path, *, prod_connection_toml: str = "") -> Path:
    database: Path = tmp_path / "warehouse.duckdb"
    _ = (tmp_path / "sqlbuild_project.toml").write_text(
        (
            'name = "contract_test"\n'
            'adapter = "duckdb"\n'
            'default_target = "dev"\n'
            f'\n[connection]\ndatabase = "{database}"\n'
            '\n[targets.dev]\nschema = "dev"\n'
            '\n[targets.prod]\nschema = "prod"\n'
            f"{prod_connection_toml}"
        ),
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    sources: Path = tmp_path / "sources"
    models.mkdir()
    sources.mkdir()
    _ = (models / "orders.sql").write_text(
        """MODEL (description "Test model orders.",
  materialized table
  -- keep model metadata
  columns (
    id ()
  )
);
SELECT CAST(1 AS INTEGER) AS id, CAST('a' AS VARCHAR) AS name
""",
        encoding="utf-8",
    )
    _ = (sources / "raw.yml").write_text(
        """sources:
  - name: raw_orders
    schema: raw
    table: orders
    description: keep source
    columns:
      - name: id
        description: identifier
""",
        encoding="utf-8",
    )
    with duckdb.connect(str(database)) as connection:
        connection.execute("CREATE SCHEMA prod")
        connection.execute("CREATE SCHEMA raw")
        connection.execute("CREATE TABLE prod.orders(id INTEGER, name VARCHAR)")
        connection.execute("CREATE TABLE raw.orders(id BIGINT, status VARCHAR)")
    return database


def add_second_contract_source(*, project_dir: Path, database: Path) -> None:
    """Add another physical source declared in the existing source YAML file."""

    source_path: Path = project_dir / "sources" / "raw.yml"
    _ = source_path.write_text(
        source_path.read_text(encoding="utf-8")
        + """  - name: raw_customers
    schema: raw
    table: customers
    description: keep second source
""",
        encoding="utf-8",
    )
    with duckdb.connect(str(database)) as connection:
        connection.execute("ALTER TABLE raw.orders ADD COLUMN generated_at TIMESTAMP")
        connection.execute("CREATE TABLE raw.customers(customer_id BIGINT, email VARCHAR)")


def compile_finding_keys(*, project_dir: Path, capsys: pytest.CaptureFixture[str]) -> set[str]:
    """Compile through the CLI and return diagnostics as path:code keys."""

    _ = main(["--project-dir", str(project_dir), "compile", "--json"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], payload["diagnostics"])
    return {f"{item['path']}:{item['code']}" for item in diagnostics}


class LiveRetentionFake:
    """Pretend every managed table currently keeps a fixed number of retention days."""

    def __init__(self, *, live_days: int) -> None:
        self.live_days: int = live_days
        self.requested_days: list[int] = []

    def inspect_retention(self, *, connection: object, request: RetentionRequest) -> RetentionState:
        del connection
        return RetentionState(
            request_id=request.request_id,
            scope=request.scope,
            configured_days=self.live_days,
            effective_days=self.live_days,
        )

    def render_retention_changes(
        self, *, request: RetentionRequest, state: RetentionState | None = None
    ) -> tuple[RenderedRetentionChange, ...]:
        del state
        self.requested_days.append(request.desired_days)
        return (
            RenderedRetentionChange(phase=RetentionChangePhase.ALTER, statements=("SELECT 1",)),
        )


def install_live_retention_fake(
    *, monkeypatch: pytest.MonkeyPatch, fake: LiveRetentionFake
) -> None:
    """Route DuckDB retention inspection and rendering through the fake."""

    monkeypatch.setattr(
        DuckDbAdapter,
        "inspect_retention",
        lambda _self, *, connection, request: fake.inspect_retention(
            connection=connection, request=request
        ),
    )
    monkeypatch.setattr(
        DuckDbAdapter,
        "render_retention_changes",
        lambda _self, *, request, state=None: fake.render_retention_changes(
            request=request, state=state
        ),
    )


def write_retention_policy_project(*, project_dir: Path, target_lines: tuple[str, ...]) -> None:
    """Write a DuckDB project with one table and one view under a prod target."""

    (project_dir / "models").mkdir(parents=True, exist_ok=True)
    _ = (project_dir / "sqlbuild_project.toml").write_text(
        "\n".join(
            (
                'name = "demo"',
                'adapter = "duckdb"',
                'default_target = "prod"',
                "",
                "[connection]",
                'database = "demo.duckdb"',
                "",
                "[materialization_defaults.table]",
                'time_travel_retention = "90d"',
                "",
                "[targets.prod]",
                *target_lines,
                "",
            )
        ),
        encoding="utf-8",
    )
    _ = (project_dir / "models" / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    _ = (project_dir / "models" / "order_view.sql").write_text(
        'MODEL (description "Test model order_view.", '
        'materialized view);\n\nSELECT order_id FROM __ref("orders")\n',
        encoding="utf-8",
    )


def run_build(
    *, project_dir: Path, flags: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> tuple[int, str]:
    """Run one real CLI build and return its exit code and combined output."""

    exit_code: int = main(["--project-dir", str(project_dir), "--no-color", "build", *flags])
    return exit_code, "".join(capsys.readouterr())


def built_model_rows(
    *, project_dir: Path, model: str, capsys: pytest.CaptureFixture[str]
) -> list[tuple[Any, ...]]:
    """Build the project into its `orders.duckdb` and return one model's rows in a stable order."""

    exit_code, output = run_build(project_dir=project_dir, flags=(), capsys=capsys)
    assert exit_code == 0, output
    with duckdb.connect(str(project_dir / "orders.duckdb"), read_only=True) as connection:
        return sorted(connection.execute(f"SELECT * FROM {model}").fetchall(), key=repr)


def rule_fix_statuses(*, payload: dict[str, Any], code: str, file_name: str) -> tuple[str, ...]:
    """Statuses of one Rule's fixes in one file, from `format --fix --json` output."""

    key: tuple[str, str] = (code, file_name)
    statuses: dict[tuple[str, str], list[str]] = {key: []}
    for fix in payload["rule_fixes"]:
        statuses.setdefault((fix["code"], Path(fix["file"]).name), []).append(fix["status"])
    return tuple(statuses[key])


def write_dropped_relation_project(
    *, project_dir: Path, model_name: str, model_sql: str, settings_toml: str = ""
) -> Path:
    """Write a DuckDB project with raw order sources, one seed, and one model."""

    (project_dir / "models").mkdir(parents=True, exist_ok=True)
    (project_dir / "sources").mkdir(parents=True, exist_ok=True)
    (project_dir / "seeds").mkdir(parents=True, exist_ok=True)
    _ = (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
        + settings_toml,
        encoding="utf-8",
    )
    _ = (project_dir / "sources" / "raw.yml").write_text(
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "  - name: raw_customers\n    description: Test source raw_customers.\n"
        "    schema: main\n"
        "    table: raw_customers\n",
        encoding="utf-8",
    )
    _ = (project_dir / "seeds" / "order_statuses.csv").write_text(
        "status_code,status_name\n1,placed\n2,shipped\n", encoding="utf-8"
    )
    _ = (project_dir / "seeds" / "lookups.yml").write_text(
        "seeds:\n"
        "  - name: order_statuses\n    description: Test seed order_statuses.\n"
        "    columns:\n"
        "      - name: status_code\n"
        "        type: INTEGER\n"
        "      - name: status_name\n"
        "        type: VARCHAR\n",
        encoding="utf-8",
    )
    _ = (project_dir / "models" / f"{model_name}.sql").write_text(model_sql, encoding="utf-8")
    db_path: Path = project_dir / "orders.duckdb"
    execute_duckdb_sql(
        db_path=db_path,
        sql=(
            "CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP); "
            "INSERT INTO main.raw_orders VALUES "
            "(1, '2026-01-01 05:00:00'), (2, '2026-01-02 01:00:00'), "
            "(3, '2026-01-03 01:00:00'); "
            "CREATE TABLE main.raw_customers AS "
            "SELECT 1 AS customer_id, 'basic' AS plan, TIMESTAMP '2026-01-01' AS updated_at"
        ),
    )
    return db_path


def execute_duckdb_sql(*, db_path: Path, sql: str) -> None:
    """Run mutating SQL directly against a DuckDB file, outside SQLBuild."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    try:
        _ = connection.execute(sql)
    finally:
        connection.close()


def query_duckdb_rows(*, db_path: Path, sql: str) -> tuple[tuple[object, ...], ...]:
    """Read rows directly from a DuckDB file."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path), read_only=True)
    try:
        return tuple(tuple(row) for row in connection.execute(sql).fetchall())
    finally:
        connection.close()


def run_plan_json(
    *, project_dir: Path, flags: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> dict[str, object]:
    """Run one real CLI JSON plan and return the parsed document."""

    _ = capsys.readouterr()
    exit_code: int = main(["--project-dir", str(project_dir), "plan", "--json", *flags])
    out: str
    err: str
    out, err = capsys.readouterr()
    assert exit_code == 0, out + err
    return cast(dict[str, object], json.loads(out))


def plan_model(*, plan: dict[str, object], name: str) -> dict[str, object]:
    """Return one serialized model entry from a JSON plan."""

    models: list[dict[str, object]] = cast(list[dict[str, object]], plan["models"])
    return {str(model["name"]): model for model in models}[name]


def plan_warning_identities(*, plan: dict[str, object]) -> tuple[tuple[object, object], ...]:
    """Return the resource name and diagnostic code of every plan warning."""

    warnings: list[dict[str, object]] = cast(list[dict[str, object]], plan["warnings"])
    return tuple((warning.get("model_name"), warning.get("code")) for warning in warnings)


def plan_cursor_bounds(*, plan: dict[str, object], name: str) -> dict[str, object]:
    """Return the serialized cursor bounds of one planned model."""

    return cast(dict[str, object], plan_model(plan=plan, name=name)["cursor_bounds"])


def plan_seed_reasons(*, plan: dict[str, object]) -> dict[str, object]:
    """Return plan reasons keyed by seed name."""

    seeds: list[dict[str, object]] = cast(list[dict[str, object]], plan["seeds"])
    return {str(seed["name"]): seed["reason"] for seed in seeds}


def dropped_relation_microbatch_sql(*, batch_concurrency: int) -> str:
    """Render an integer-cursor microbatch model over the raw order source."""

    return (
        "MODEL (description 'Test model.',\n"
        "  materialized incremental,\n"
        "  incremental_strategy delete_insert,\n"
        "  incremental_mode microbatch,\n"
        "  microbatch_strategy watermark,\n"
        "  cursor_watermark_mode all,\n"
        "  cursor id,\n"
        "  cursor_type integer,\n"
        "  cursor_inputs (raw_orders (column id, roles [filter, watermark]),),\n"
        '  batch_size "1",\n'
        f"  batch_concurrency {batch_concurrency},\n"
        ");\n\n"
        'SELECT id, ordered_at FROM __source("raw_orders")\n'
        "WHERE id >= __cursor_start() AND id < __cursor_end()\n"
    )


def compile_duckdb_query(
    *, tmp_path: Path, capsys: pytest.CaptureFixture[str], query: str
) -> tuple[int, tuple[object, ...]]:
    """Compile one DuckDB model query and return the exit and diagnostic codes."""

    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(f"MODEL (description 'Test model orders.');\n{query}")
    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])
    return exit_code, tuple(item["code"] for item in diagnostics)


def write_expression_source_project(
    *, project_dir: Path, source_expression: str, model_sql: str
) -> None:
    """Write a DuckDB project with one typed expression source and one reading model."""

    (project_dir / "sources").mkdir(parents=True)
    (project_dir / "models").mkdir()
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[connection]\ndatabase = "orders.duckdb"\n',
        encoding="utf-8",
    )
    (project_dir / "sources" / "orders.yml").write_text(
        "sources:\n"
        "  - name: typed_orders\n    description: Test source typed_orders.\n"
        f'    expression: "{source_expression}"\n'
        "    columns:\n"
        "      - name: order_id\n"
        "        type: INTEGER\n",
        encoding="utf-8",
    )
    (project_dir / "models" / "orders.sql").write_text(model_sql, encoding="utf-8")


def source_rebinding_analyses() -> int:
    """Model queries analysed again against inspected source columns since the last reset."""

    return native_module.compile_stage_work_info().get("source_rebinding_analyses", 0)


def eager_output_column_scans(*, capture_dir: Path) -> tuple[int, int]:
    """Count the captured model files, and those discovery located output columns for eagerly."""

    located: list[bool] = []

    def record(item: dict[str, object]) -> dict[str, object]:
        located.extend(map(bool, map(item.get, filter(item.__contains__, _OUTPUT_LOCATIONS))))
        return item

    for capture in sorted(
        capture_dir.glob(f"*-{CompilerStage.DISCOVERED_PROJECT_INPUTS.value}{STAGE_CAPTURE_SUFFIX}")
    ):
        _ = json.loads(capture.read_text(encoding="utf-8"), object_hook=record)
    return len(located), sum(located)


_OUTPUT_LOCATIONS: tuple[str, ...] = ("output_column_locations",)


def write_repeated_json_parse_project(
    *, tmp_path: Path, test_case: RepeatedJsonParseTestCase
) -> None:
    """Write a one-model project that selects only SQLCLEAN045."""

    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.adapter}"\n\n[rules]\nselect = ["SQLCLEAN045"]\n',
        encoding="utf-8",
    )
    files: tuple[tuple[str, str], ...] = (
        ("models/order_events.sql", f"{test_case.header}{test_case.sql}"),
        *test_case.extra_files,
    )
    for relative_path, contents in files:
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


class OfflineSnowflakeRawConnection:
    """Snowflake connector double that answers reads with no rows and refuses writes."""

    description: None = None
    rowcount: int = 0
    sfqid: str = "offline"

    def __init__(self, executed_sql: list[str]) -> None:
        self._executed_sql: list[str] = executed_sql

    def cursor(self, *args: object, **kwargs: object) -> OfflineSnowflakeRawConnection:
        del args, kwargs
        return self

    def execute(self, sql: str, *args: object, **kwargs: object) -> OfflineSnowflakeRawConnection:
        del args, kwargs
        self._executed_sql.append(sql)
        _OFFLINE_SNOWFLAKE_STATEMENT_RESPONSES.get(
            sql.lstrip().split(None, 1)[0].upper(), _refuse_offline_snowflake_write
        )()
        return self

    def fetchall(self) -> list[tuple[object, ...]]:
        return []

    def fetchone(self) -> None:
        return None

    def close(self) -> None:
        pass


def _accept_offline_snowflake_read() -> None:
    return None


def _refuse_offline_snowflake_write() -> None:
    raise AdapterUserError(message="offline Snowflake double refuses writes")


_OFFLINE_SNOWFLAKE_STATEMENT_RESPONSES: dict[str, Callable[[], None]] = dict.fromkeys(
    ("USE", "SHOW", "SELECT", "DESC", "DESCRIBE", "WITH"), _accept_offline_snowflake_read
)


def install_offline_snowflake_connector(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[list[dict[str, object]], list[str]]:
    """Install an offline snowflake.connector and capture every connect call and statement."""

    connect_calls: list[dict[str, object]] = []
    executed_sql: list[str] = []
    snowflake_module: ModuleType = ModuleType("snowflake")
    connector_module: ModuleType = ModuleType("snowflake.connector")

    def connect(**kwargs: object) -> OfflineSnowflakeRawConnection:
        connect_calls.append(kwargs)
        return OfflineSnowflakeRawConnection(executed_sql)

    connector_module.__dict__["connect"] = connect
    snowflake_module.__dict__["connector"] = connector_module
    monkeypatch.setitem(sys.modules, "snowflake", snowflake_module)
    monkeypatch.setitem(sys.modules, "snowflake.connector", connector_module)
    return connect_calls, executed_sql


def write_snowflake_warehouse_project(
    *, project_dir: Path, warehouses_section: str, local_config: str
) -> None:
    """Write a one-model Snowflake project whose connection names a fallback warehouse."""

    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\n'
        'adapter = "snowflake"\n'
        'default_target = "dev"\n\n'
        "[connections.main]\n"
        'account = "example-account"\n'
        'user = "builder"\n'
        'password = "example-password"\n'
        'warehouse = "ANALYTICS_WH"\n\n'
        "[targets.dev]\n"
        'connection = "main"\n'
        'database = "ANALYTICS"\n'
        'schema = "DEV"\n\n' + warehouses_section,
        encoding="utf-8",
    )
    (project_dir / "sqlbuild_local.toml").write_text(local_config, encoding="utf-8")
    (project_dir / "models").mkdir()
    (project_dir / "models" / "orders.sql").write_text(
        "MODEL (description 'Orders.', materialized table);\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )


def write_default_connection_project(*, project_dir: Path, project_toml: str) -> None:
    """Write a one-model DuckDB project with the given project config."""

    (project_dir / "models").mkdir()
    _ = (project_dir / "sqlbuild_project.toml").write_text(project_toml, encoding="utf-8")
    _ = (project_dir / "models" / "orders.sql").write_text(
        'MODEL (description "Orders.", materialized table);\n\nSELECT 1 AS order_id\n',
        encoding="utf-8",
    )


def write_default_connection_sql_test(*, project_dir: Path) -> None:
    """Add a downstream model and a SQL test that mocks its orders dependency."""

    _ = (project_dir / "models" / "order_totals.sql").write_text(
        'MODEL (description "Order totals.", materialized table);\n\n'
        'SELECT order_id FROM __ref("orders")\n',
        encoding="utf-8",
    )
    (project_dir / "tests" / "unit").mkdir(parents=True)
    _ = (project_dir / "tests" / "unit" / "test_order_totals.sql").write_text(
        'TEST (name "order_totals_value");\n\n'
        "WITH __ref__orders AS (SELECT 1 AS order_id),\n"
        "__expected__order_totals AS (SELECT 1 AS order_id)\n"
        "SELECT 1\n",
        encoding="utf-8",
    )


def write_project_files(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write project files at their project-relative paths."""

    for relative, contents in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")


EARLY_LINT_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[vars]\nfloor = "2"\n\n'
        '[rules]\nselect = ["SQLCLEAN005"]\n'
    ),
    "models/_macros/python/cents.py": (
        'def cents(expression: str) -> str:\n    """Convert to cents."""\n'
        '    return f"({expression} * 100)"\n'
    ),
    "models/orders.sql": (
        'MODEL (description "Orders");\nSELECT 1 AS order_id, CAST(5 AS DOUBLE) AS amount\n'
    ),
    "models/order_cents.sql": (
        'MODEL (description "Order amounts in cents");\n'
        'WITH floored AS (SELECT order_id, @py.cents("amount") AS amount_cents, '
        '@@floor AS floor_amount FROM __ref("orders")), spare AS (SELECT 1 AS one)\n'
        "SELECT order_id, amount_cents FROM floored WHERE amount_cents > floor_amount\n"
    ),
}


def compile_findings(
    *,
    project_dir: Path,
    capsys: pytest.CaptureFixture[str],
    flags: tuple[str, ...] = ("--no-cache",),
) -> tuple[tuple[str, str, int | None, int | None], ...]:
    """Compile through the CLI, by default with no cache, and return each finding's location."""

    _ = main(["--project-dir", str(project_dir), "compile", "--json", *flags])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, Any]] = cast(list[dict[str, Any]], payload["diagnostics"])
    return tuple(
        (item["code"], item["path"], item.get("line"), item.get("column")) for item in diagnostics
    )


CUSTOM_HOST_RULE_CODE: str = "XSQBRHOST001"


def write_custom_host_project(*, project_dir: Path, model_count: int, rule_body: str) -> None:
    """Write numbered order models and one custom model rule running `rule_body`."""

    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n'
        f'[rules]\nselect = ["{CUSTOM_HOST_RULE_CODE}"]\n\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
        encoding="utf-8",
    )
    for index in range(model_count):
        model: Path = project_dir / "models" / f"orders_{index:03d}.sql"
        model.parent.mkdir(parents=True, exist_ok=True)
        model.write_text(
            f'MODEL (description "Orders {index}");\nSELECT {index} AS order_id\n',
            encoding="utf-8",
        )
    body: str = "".join(f"    {line}\n" for line in rule_body.splitlines())
    rule_file: Path = project_dir / "rules" / "orders.py"
    rule_file.parent.mkdir(parents=True, exist_ok=True)
    rule_file.write_text(
        "from sqlbuild.rules import Finding, Model, RuleContext, rule\n\n\n"
        f'@rule(code="{CUSTOM_HOST_RULE_CODE}", message="orders", remediation="Fix orders.")\n'
        "def check_orders(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        f"{body}",
        encoding="utf-8",
    )


def interrupt_main_after(delay_seconds: float) -> Callable[[Path], None]:
    """Return a host publish hook that interrupts the main thread after a delay."""

    def interrupt(path: Path) -> None:
        del path
        threading.Timer(delay_seconds, _thread.interrupt_main).start()

    return interrupt


def child_processes() -> frozenset[int]:
    """Return the live child processes of this process."""

    children: set[int] = set()
    for task in (Path("/proc") / str(os.getpid()) / "task").iterdir():
        try:
            listed: str = (task / "children").read_text()
        except FileNotFoundError:
            continue  # the thread exited after the task list was read
        children.update(map(int, listed.split()))
    return frozenset(children)


def observe_hosts(
    *,
    monkeypatch: pytest.MonkeyPatch,
    cores: int,
    publish_inputs: Callable[[Path], None] | None = None,
) -> CustomHostObserver:
    """Run custom rules on up to `cores` hosts, one invocation each, and record every launch."""

    observer: CustomHostObserver = CustomHostObserver(
        cores=cores, min_invocations_per_host=1, publish_inputs=publish_inputs
    )
    monkeypatch.setattr(native_module, "custom_host_observer", observer)
    return observer


PROVIDER_SOURCE: str = """
from sqlbuild.providers import Provider


class OrdersProvider(Provider):
    'Order service settings.'

    region: str = "eu"
"""


def write_provider_task_project(*, project_dir: Path, annotation: str) -> None:
    """Write a DuckDB project whose task takes a provider parameter with `annotation`."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "orders_tasks"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "orders.duckdb"\n'
        ),
        "providers/orders.py": PROVIDER_SOURCE,
        "python/orders.py": (
            "from providers.orders import OrdersProvider\n"
            "from sqlbuild.tasks import task\n\n\n"
            "@task\n"
            f"def refresh_orders(ctx, orders_provider: {annotation}):\n"
            '    """Refresh the order snapshot."""\n'
            "    return None\n"
        ),
    }
    for relative_path, text in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def write_header_project(project_dir: Path, models: dict[str, str], project_toml: str) -> None:
    """Write a DuckDB project with one SQL model file per name."""

    _ = (project_dir / "sqlbuild_project.toml").write_text(project_toml)
    model_dir: Path = project_dir / "models"
    model_dir.mkdir()
    for name, contents in models.items():
        _ = (model_dir / f"{name}.sql").write_text(contents)


def compile_diagnostics(
    project_dir: Path, capsys: pytest.CaptureFixture[str]
) -> tuple[int, list[dict[str, object]]]:
    """Run one real CLI JSON compile and return its exit code and diagnostics."""

    exit_code: int = main(["--project-dir", str(project_dir), "compile", "--json", "--no-cache"])
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    return exit_code, cast(list[dict[str, object]], payload["diagnostics"])


def diagnostics_by_model(diagnostics: list[dict[str, object]]) -> dict[str, str]:
    """Each model's diagnostic as JSON with its own name replaced, to compare across models."""

    by_model: dict[str, str] = {}
    for item in diagnostics:
        name: str = cast(str, item["resource_name"])
        by_model[name] = json.dumps(item, sort_keys=True).replace(name, "<model>")
    return by_model


def typed_config_reads(*, project_dir: Path, extra_header: str) -> int:
    """Config conversions for a fresh build, an inherited build and a reused config."""

    project_dir.mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connections.local]\ndatabase = ":memory:"\n\n[defaults]\nmaterialized = "table"\n\n'
        '[targets.dev]\nschema = "main"\n',
        encoding="utf-8",
    )
    models_dir: Path = project_dir / "models"
    models_dir.mkdir()
    for index, own_config in enumerate((', materialized table, tags ["daily"]', "", "")):
        (models_dir / f"orders_{index}.sql").write_text(
            f'MODEL (description "Orders {index}."{own_config}{extra_header});\n\n'
            f"SELECT {index} AS order_id, 1 AS loaded_at\n",
            encoding="utf-8",
        )
    _ = native_module.compile_stage_work_info(reset=True)
    exit_code: int = main(["--project-dir", str(project_dir), "plan", "--json", "--no-cache"])
    assert exit_code == 0
    return native_module.compile_stage_work_info(reset=True).get("typed_config_reads", 0)


def compiled_view_fallbacks(
    *, project_dir: Path, write_project: Callable[[Path], None], command: tuple[str, ...]
) -> tuple[int, int]:
    """The exit code of `command` over a native project and its reads that missed a native view."""

    write_project(project_dir)
    _ = native_module.compile_stage_work_info(reset=True)
    exit_code: int = main(["--project-dir", str(project_dir), *command])
    fallbacks: int = native_module.compile_stage_work_info(reset=True).get(
        "compiled_view_fallbacks", 0
    )
    return exit_code, fallbacks


def copy_waffle_shop(project_dir: Path) -> None:
    """Copy the waffle shop fixture, with sources, seeds, functions and audits, to `project_dir`."""

    _ = shutil.copytree(E2E_FIXTURES_DIR / "waffle_shop", project_dir)


def write_orders_project(project_dir: Path) -> None:
    """Write a three-model orders project to `project_dir`."""

    project_dir.mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connections.local]\ndatabase = ":memory:"\n\n[targets.dev]\nschema = "main"\n',
        encoding="utf-8",
    )
    models_dir: Path = project_dir / "models"
    models_dir.mkdir()
    (models_dir / "orders.sql").write_text(
        'MODEL (description "Orders.", materialized table, contract enforced, '
        'columns (order_id (type "INTEGER"), loaded_at (type "INTEGER")));\n\n'
        "SELECT CAST(1 AS INTEGER) AS order_id, CAST(1 AS INTEGER) AS loaded_at\n",
        encoding="utf-8",
    )
    (models_dir / "daily_orders.sql").write_text(
        'MODEL (description "Orders per day.", materialized table);\n\n'
        'SELECT order_id, loaded_at FROM __ref("orders")\n',
        encoding="utf-8",
    )
    (models_dir / "order_view.sql").write_text(
        'MODEL (description "Order ids.", materialized view);\n\n'
        'SELECT order_id FROM __ref("daily_orders")\n',
        encoding="utf-8",
    )


FORMAT_CACHE_UNFORMATTED_MODELS: dict[str, str] = {
    "orders.sql": (
        'MODEL (description "Orders placed by customers.", materialized table);\n'
        "select   order_id, customer_id from __source('shop', 'orders')\n"
    ),
    "customers.sql": (
        'MODEL (description "Customers who placed orders.", materialized table);\n'
        "select customer_id,   min(order_id) as first_order_id from __ref('orders') "
        "group by customer_id\n"
    ),
    "products.sql": (
        'MODEL (description "Products in the catalog.", materialized view);\n'
        "select 1 as product_id, 'widget' as product_name\n"
    ),
}


def write_formatted_cache_project(*, project_dir: Path) -> None:
    """Write three small models and format them once without the format result cache."""

    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    models_dir: Path = project_dir / "models"
    models_dir.mkdir()
    name: str
    sql: str
    for name, sql in FORMAT_CACHE_UNFORMATTED_MODELS.items():
        (models_dir / name).write_text(sql, encoding="utf-8")
    assert main(["--project-dir", str(project_dir), "format", "--no-cache"]) == 0


def format_result_store_path(*, project_dir: Path) -> Path:
    """The project's format result store file."""

    return (
        native_module.compiler_cache_directory(project_dir)
        / native_module.FORMAT_RESULT_STORE_FILE_NAME
    )


def check_format_with_work(
    *, project_dir: Path, arguments: tuple[str, ...] = ()
) -> tuple[int, dict[str, int]]:
    """Run `format --check` in process and return its exit code and native stage work."""

    _ = native_module.compile_stage_work_info(reset=True)
    exit_code: int = main(["--project-dir", str(project_dir), "format", "--check", *arguments])
    return exit_code, native_module.compile_stage_work_info()


def edit_cached_orders_model(project_dir: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Make one cached model unformatted again."""

    (project_dir / "models" / "orders.sql").write_text(
        FORMAT_CACHE_UNFORMATTED_MODELS["orders.sql"], encoding="utf-8"
    )


def widen_format_line_width(project_dir: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Change the `[format]` line width, which every cache key includes."""

    config: Path = project_dir / "sqlbuild_project.toml"
    _ = config.write_text(
        config.read_text(encoding="utf-8") + "\n[format]\nline_width = 120\n", encoding="utf-8"
    )


def damage_format_result_store(project_dir: Path, _monkeypatch: pytest.MonkeyPatch) -> None:
    """Overwrite the end of the stored format results so the store fails its checksum."""

    path: Path = format_result_store_path(project_dir=project_dir)
    assert path.is_file()
    _ = path.write_bytes(path.read_bytes()[:-7] + b"damaged")


def rebuild_formatter(_project_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Pretend another SQLBuild build formats the project."""

    rebuild_identity(monkeypatch=monkeypatch)


PROJECT_SCRATCH_RULES: str = """from sqlbuild.rules import Finding, Project, RuleContext, rule

SCRATCH: list[str] = []


@rule(code="XSQBRSCR001", message="Stage names", remediation="Keep rules pure.")
def stage(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    SCRATCH.extend(model.name for model in ctx.project.models)
    return []


@rule(code="XSQBRSCR002", message="Staged names are present", remediation="Keep rules pure.")
def read(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    return [ctx.finding(subject=ctx.project.models[0])] * int(bool(SCRATCH))


@rule(code="XSQBRSCR003", message="Clear staged names", remediation="Keep rules pure.")
def clear(*, project: Project, ctx: RuleContext) -> list[Finding]:
    del project
    SCRATCH.clear()
    return []
"""


def write_project_scratch_rules(project_dir: Path) -> None:
    """Write two order models and three project rules sharing one scratch list."""

    write_project_files(
        project_dir=project_dir,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["XSQBRSCR"]\n\n'
                "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
            ),
            "models/orders.sql": 'MODEL (description "Orders");\nSELECT 1 AS order_id\n',
            "models/customers.sql": ('MODEL (description "Customers");\nSELECT 1 AS customer_id\n'),
            "rules/scratch.py": PROJECT_SCRATCH_RULES,
        },
    )


def write_enum_project(*, root: Path, model_sql: str, contract: str) -> None:
    """Write an orders project whose model fills an `order_status` column from `model_sql`."""

    (root / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    (root / "models" / "_enums").mkdir(parents=True)
    (root / "models" / "_enums" / "order_status.sql").write_text(
        'ENUM (name order_status, members (OPEN "open", CLOSED "closed"));\n'
    )
    (root / "sources").mkdir()
    (root / "sources" / "orders.yml").write_text(
        "sources:\n"
        "  - name: web_orders\n    description: Web orders.\n    contract: enforced\n"
        '    expression: "(SELECT CAST(1 AS INTEGER) AS order_id)"\n'
        "    columns:\n      - name: order_id\n        type: INTEGER\n"
        "  - name: partner_orders\n    description: Partner orders.\n    contract: enforced\n"
        '    expression: "(SELECT CAST(2 AS INTEGER) AS order_id)"\n'
        "    columns:\n      - name: order_id\n        type: INTEGER\n"
    )
    (root / "models" / "orders.sql").write_text(
        f"MODEL (\n  description 'Orders from every channel.',\n  contract {contract},\n"
        "  columns (\n    order_id (type INTEGER),\n    status (type order_status),\n  ),\n);\n\n"
        f"{model_sql}"
    )


def _weekly_rates(first_step: str) -> str:
    return (
        'MACRO (\n  description "Weekly refund rate per grain",\n'
        "  arguments (weekly RELATION (keys COLUMNS, order_count BIGINT, refund_count BIGINT)),\n"
        ");\n\n"
        f"WITH counted AS (\n{first_step}\n),\n\n"
        "rated AS (\n"
        "  SELECT\n"
        "    @weekly.keys,\n"
        "    SUM(refunds) / NULLIF(SUM(orders), 0) AS refund_rate\n"
        "  FROM counted\n"
        "  GROUP BY @weekly.keys\n"
        ")\n"
    )


def refund_rates_project(first_step: str) -> dict[str, str]:
    """Weekly orders, refund-rate models and the SQL macro whose first CTE is `first_step`."""

    return {
        "models/weekly_orders.sql": _WEEKLY_ORDERS,
        "models/refund_rates.sql": _REFUND_RATES,
        "models/_macros/sql/weekly_rates.sql": _weekly_rates(first_step),
    }


def open_orders_model(sql: str) -> str:
    """An `open_orders` table model whose query is `sql`."""

    return f'MODEL (\n  description "Open orders",\n  materialized table,\n);\n\n{sql}\n'


_ENDLESS_ORDER_TOTAL: str = "(SELECT SUM(big.n) FROM range(1000000000000) AS big(n)) * 0"


def write_endless_scenario_project(project_dir: Path) -> Path:
    """A DuckDB project whose scenario builds a model that runs until it is cancelled."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "scenario_signals"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "scenario_signals.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n"
            "    description: Raw orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
        ),
        "models/order_totals.sql": (
            "MODEL (description 'Order totals.', materialized table, sql_analysis false);\n\n"
            f"SELECT SUM(o.amount) + {_ENDLESS_ORDER_TOTAL} AS total_amount "
            'FROM __source("raw_orders") AS o\n'
        ),
        "tests/scenarios/totals.sql": (
            "SCENARIO (description 'Order totals add up.');\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS id, 10 AS amount),\n"
            "__expected__order_totals AS (SELECT 10 AS total_amount)\n"
            "SELECT 1\n"
        ),
    }
    for relative, contents in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    return project_dir


def signalled_scenario_run(
    *, project_dir: Path, sent_signal: signal.Signals
) -> tuple[int, str, float]:
    """Exit code and output of `sqb scenario test` sent `sent_signal` while its model runs, and
    the seconds it took to stop."""

    process: subprocess.Popen[str] = subprocess.Popen(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--no-color",
            "--project-dir",
            str(project_dir),
            "scenario",
            "test",
            "--concurrency",
            "1",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env={**os.environ, "PYTHONUNBUFFERED": "1"},
        preexec_fn=_default_interrupt_handling,
    )
    assert process.stdout is not None
    started: str = "".join(takewhile(lambda line: "order_totals START" not in line, process.stdout))
    time.sleep(1.0)
    process.send_signal(sent_signal)
    sent: float = time.monotonic()
    stdout, stderr = process.communicate(timeout=120)
    return process.returncode, started + stdout + stderr, time.monotonic() - sent


def _default_interrupt_handling() -> None:
    """Give the spawned command default Ctrl-C handling even when its parent ignores SIGINT."""

    _ = signal.signal(signal.SIGINT, signal.SIG_DFL)
