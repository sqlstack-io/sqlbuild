"""Integration coverage for target-backed contract adoption."""

from __future__ import annotations

import json
from pathlib import Path

import duckdb
import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    ContractCommandIntegrationTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    add_second_contract_source,
    prepare_contract_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given physical drift when diffing contracts then reports without writing",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_physical_drift_when_diffing_contracts_then_reports_without_writing(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    database: Path = prepare_contract_project(tmp_path)
    original_model: str = (tmp_path / "sql" / "models" / "orders.sql").read_text(encoding="utf-8")

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "diff",
            "--from",
            "prod",
            "--select",
            "orders",
            "source:raw_orders",
        ]
    )

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "missing_declaration" in output
    assert "model:orders" in output
    assert "source:raw_orders" in output
    assert (tmp_path / "sql" / "models" / "orders.sql").read_text(
        encoding="utf-8"
    ) == original_model
    assert database.is_file()


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given missing declarations when generating additively then preserves metadata",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_declarations_when_generating_additively_then_preserves_metadata(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "source:raw_orders",
            "--write",
        ]
    )

    assert exit_code == test_case.expected_exit_code
    assert "0 contract difference(s)" in capsys.readouterr().out
    model_sql: str = (tmp_path / "sql" / "models" / "orders.sql").read_text(encoding="utf-8")
    source_yaml: str = (tmp_path / "sql" / "sources" / "raw.yml").read_text(encoding="utf-8")
    assert "id (type INTEGER)" in model_sql
    assert "name (type VARCHAR)" in model_sql
    assert "-- keep model metadata" in model_sql
    assert "description: keep source" in source_yaml
    assert "description: identifier" in source_yaml
    assert "type: 'BIGINT'" in source_yaml
    assert "- name: status" in source_yaml
    assert "type: 'VARCHAR'" in source_yaml


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="generated contracts in a commented header re-compile cleanly",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_contracts_when_compiling_then_project_compiles_cleanly(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)
    generate_exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "source:raw_orders",
            "--write",
        ]
    )
    assert generate_exit_code == test_case.expected_exit_code
    _ = capsys.readouterr()

    compile_exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile"])

    captured: CaptureResult[str] = capsys.readouterr()
    assert compile_exit_code == test_case.expected_exit_code, captured.out + captured.err
    assert "error[" not in captured.out + captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="parameterized physical type is unquoted in a generated model header",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_parameterized_physical_type_when_generating_then_writes_valid_model_header(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    database: Path = prepare_contract_project(tmp_path)
    model_path: Path = tmp_path / "sql" / "models" / "orders.sql"
    _ = model_path.write_text(
        "MODEL (description 'Test model orders.', materialized table);\n"
        "SELECT CAST(1 AS INTEGER) AS id, CAST(1 AS DECIMAL(10,2)) AS amount\n",
        encoding="utf-8",
    )
    with duckdb.connect(str(database)) as connection:
        connection.execute("DROP TABLE prod.orders")
        connection.execute("CREATE TABLE prod.orders(id INTEGER, amount DECIMAL(10,2))")

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "--write",
        ]
    )

    assert exit_code == test_case.expected_exit_code
    assert "0 contract difference(s)" in capsys.readouterr().out
    generated_sql: str = model_path.read_text(encoding="utf-8")
    assert "amount (type DECIMAL(10,2))" in generated_sql
    _ = model_path.write_text(
        generated_sql.replace(
            "amount (type DECIMAL(10,2))",
            'amount (description "type DECIMAL(10,2)", type DECIMAL (10, 2))',
        ),
        encoding="utf-8",
    )

    with duckdb.connect(str(database)) as connection:
        connection.execute("DROP TABLE prod.orders")
        connection.execute("CREATE TABLE prod.orders(id INTEGER, amount DECIMAL(12,3))")
    overwrite_exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "--write",
            "--overwrite",
        ]
    )

    assert overwrite_exit_code == test_case.expected_exit_code
    assert "0 contract difference(s)" in capsys.readouterr().out
    overwritten_sql: str = model_path.read_text(encoding="utf-8")
    assert 'amount (description "type DECIMAL(10,2)", type DECIMAL(12,3))' in overwritten_sql


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="multiple sources sharing one YAML file retain every generated edit",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_sources_in_one_file_when_generating_then_all_edits_are_retained(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    database: Path = prepare_contract_project(tmp_path)
    add_second_contract_source(project_dir=tmp_path, database=database)

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "source:raw_orders",
            "source:raw_customers",
            "--write",
        ]
    )

    source_yaml: str = (tmp_path / "sql" / "sources" / "raw.yml").read_text(encoding="utf-8")
    assert exit_code == test_case.expected_exit_code
    assert "0 contract difference(s)" in capsys.readouterr().out
    assert "type: 'BIGINT'" in source_yaml
    assert "- name: generated_at" in source_yaml
    assert "- name: status" in source_yaml
    assert "- name: customer_id" in source_yaml
    assert "- name: email" in source_yaml


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given conflicting contract when overwriting then physical shape replaces only physical facts",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_conflicting_contract_when_overwriting_then_physical_shape_replaces_only_physical_facts(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)
    model_path: Path = tmp_path / "sql" / "models" / "orders.sql"
    _ = model_path.write_text(
        """MODEL (description "Test model orders.",
  materialized table
  columns (
    id (type BIGINT, description "identifier")
    obsolete (type VARCHAR, description "remove me")
  )
);
SELECT CAST(1 AS INTEGER) AS id, CAST('a' AS VARCHAR) AS name
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "--write",
            "--overwrite",
        ]
    )

    assert exit_code == test_case.expected_exit_code
    assert "0 contract difference(s)" in capsys.readouterr().out
    model_sql: str = model_path.read_text(encoding="utf-8")
    assert 'id (type INTEGER, description "identifier")' in model_sql
    assert "obsolete" not in model_sql
    assert "name (type VARCHAR)" in model_sql


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given prod target credentials when contract diffing then uses active connection only",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_prod_target_credentials_when_contract_diffing_then_uses_active_connection_only(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(
        tmp_path,
        prod_connection_toml=(
            "\n[targets.prod.connection]\n"
            'database = "/directory/that/must/not/be/opened/prod.duckdb"\n'
        ),
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "contract",
            "diff",
            "--from",
            "prod",
            "--select",
            "orders",
            "--json",
        ]
    )

    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert exit_code == test_case.expected_exit_code
    assert payload["from_target"] == "prod"
    assert payload["resources"][0]["relation"].endswith("prod.orders")
    assert all("connection" not in key for key in payload)
    assert "Connecting to duckdb..." in captured.err
    assert "Warehouse connected" in captured.err
    assert "Inspecting 1 contract from target 'prod'..." in captured.err
    assert "✓ 1 contract from target 'prod' inspected" in captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="unavailable inspection target credentials do not block diff",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unavailable_inspection_target_credentials_when_diffing_then_uses_active_connection(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    missing_variable: str = "SQLBUILD_TEST_UNAVAILABLE_PROD_DATABASE"
    monkeypatch.delenv(missing_variable, raising=False)
    _ = prepare_contract_project(
        tmp_path,
        prod_connection_toml=(
            'connection = "prod_inspection"\n\n'
            "[connections.prod_inspection]\n"
            f'database = "${{ENV:{missing_variable}}}"\n'
        ),
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "contract",
            "diff",
            "--from",
            "prod",
            "--select",
            "orders",
            "--json",
        ]
    )

    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, object] = json.loads(captured.out)
    assert exit_code == test_case.expected_exit_code
    assert payload["from_target"] == "prod"
    assert payload["resources"][0]["relation"].endswith("prod.orders")
    assert missing_variable not in captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="unavailable inspection target credentials do not block write validation",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unavailable_inspection_target_credentials_when_generating_then_validates_with_active_connection(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    missing_variable: str = "SQLBUILD_TEST_UNAVAILABLE_PROD_DATABASE"
    monkeypatch.delenv(missing_variable, raising=False)
    _ = prepare_contract_project(
        tmp_path,
        prod_connection_toml=(
            'connection = "prod_inspection"\n\n'
            "[connections.prod_inspection]\n"
            f'database = "${{ENV:{missing_variable}}}"\n'
        ),
    )
    model_path: Path = tmp_path / "sql" / "models" / "orders.sql"

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "--write",
        ]
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code
    assert "0 contract difference(s)" in captured.out
    assert "id (type INTEGER)" in model_path.read_text(encoding="utf-8")
    assert missing_variable not in captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given shared schema when one binding differs then write fails closed",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_shared_schema_when_one_binding_differs_then_write_fails_closed(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)
    schema_path: Path = tmp_path / "sql" / "schemas" / "order.sql"
    schema_path.parent.mkdir(parents=True)
    _ = schema_path.write_text(
        "SCHEMA (name order, columns (id (type INTEGER)));\n",
        encoding="utf-8",
    )
    model_path: Path = tmp_path / "sql" / "models" / "orders.sql"
    _ = model_path.write_text(
        "MODEL (materialized table, model_schema order);\n"
        "SELECT CAST(1 AS INTEGER) AS id, CAST('a' AS VARCHAR) AS name\n",
        encoding="utf-8",
    )
    original_schema: str = schema_path.read_text(encoding="utf-8")

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "--write",
        ]
    )

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "ownership_conflict" in output
    assert "shared SCHEMA" in output
    assert schema_path.read_text(encoding="utf-8") == original_schema


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given source conflicts when overwriting then preserves retained metadata",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_conflicts_when_overwriting_then_preserves_retained_metadata(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)
    source_path: Path = tmp_path / "sql" / "sources" / "raw.yml"
    _ = source_path.write_text(
        """sources:
  - name: raw_orders
    schema: raw
    table: orders
    description: keep source
    columns:
      - name: id
        type: VARCHAR # keep comment
        description: identifier
      - name: obsolete
        type: INTEGER
        description: remove this metadata
""",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "source:raw_orders",
            "--write",
            "--overwrite",
        ]
    )

    assert exit_code == test_case.expected_exit_code
    _ = capsys.readouterr()
    source_yaml: str = source_path.read_text(encoding="utf-8")
    assert "type: 'BIGINT' # keep comment" in source_yaml
    assert "description: identifier" in source_yaml
    assert "obsolete" not in source_yaml
    assert "description: keep source" in source_yaml
    assert "- name: status" in source_yaml


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given cli vars when writing then validation recompile uses same values",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cli_vars_when_writing_then_validation_recompile_uses_same_values(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)
    model_path: Path = tmp_path / "sql" / "models" / "orders.sql"
    _ = model_path.write_text(
        "MODEL (description 'Test model orders.', materialized table);\n"
        "SELECT CAST(@@identifier AS INTEGER) AS id, CAST('a' AS VARCHAR) AS name\n",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "--write",
            "--vars",
            '{"identifier": 1}',
        ]
    )

    assert exit_code == test_case.expected_exit_code
    assert "id (type INTEGER)" in model_path.read_text(encoding="utf-8")
    _ = capsys.readouterr()


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="given unresolved additive type conflict when writing then file is not reported written",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unresolved_additive_type_conflict_when_writing_then_file_is_not_reported_written(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)
    model_path: Path = tmp_path / "sql" / "models" / "orders.sql"
    _ = model_path.write_text(
        "MODEL (description 'Test model orders.', materialized table, columns (id (type BIGINT), name (type VARCHAR)));\n"
        "SELECT CAST(1 AS INTEGER) AS id, CAST('a' AS VARCHAR) AS name\n",
        encoding="utf-8",
    )
    original: str = model_path.read_text(encoding="utf-8")

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "orders",
            "--write",
        ]
    )

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "Updated repository declarations" not in output
    assert model_path.read_text(encoding="utf-8") == original


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="relative project directory preserves private macro scope",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_relative_project_directory_when_diffing_contract_then_private_macro_is_visible(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / "project"
    project_dir.mkdir(parents=True)
    _ = prepare_contract_project(project_dir)
    root_model_path: Path = project_dir / "sql" / "models" / "orders.sql"
    root_model_path.unlink()
    model_path: Path = project_dir / "sql" / "models" / "orders" / "orders.sql"
    model_path.parent.mkdir(parents=True)
    _ = model_path.write_text(
        "MODEL (description 'Test model orders.', materialized table);\nSELECT @py.order_id() AS id, CAST('a' AS VARCHAR) AS name\n",
        encoding="utf-8",
    )
    macro_path: Path = model_path.parent / "_sqlbuild" / "_macros" / "python" / "order_id.py"
    macro_path.parent.mkdir(parents=True)
    _ = macro_path.write_text(
        'def order_id() -> str:\n    return "CAST(1 AS INTEGER)"\n', encoding="utf-8"
    )
    monkeypatch.chdir(tmp_path)

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            project_dir.name,
            "contract",
            "diff",
            "--from",
            "prod",
            "--select",
            "orders",
        ]
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code
    assert "model:orders" in captured.out
    assert "inaccessible" not in captured.err


@pytest.mark.parametrize(
    "test_case",
    [
        ContractCommandIntegrationTestCase(
            description="indentless source column sequence receives physical types",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_indentless_source_columns_when_generating_then_preserves_metadata_and_adds_types(
    test_case: ContractCommandIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = prepare_contract_project(tmp_path)
    source_path: Path = tmp_path / "sql" / "sources" / "raw.yml"
    _ = source_path.write_text(
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

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "contract",
            "generate",
            "--from",
            "prod",
            "--select",
            "source:raw_orders",
            "--write",
        ]
    )

    source_yaml: str = source_path.read_text(encoding="utf-8")
    assert exit_code == test_case.expected_exit_code
    assert "0 contract difference(s)" in capsys.readouterr().out
    assert "description: keep source" in source_yaml
    assert "description: identifier" in source_yaml
    assert "    - name: id\n      type: 'BIGINT'" in source_yaml
    assert "    - name: status\n      type: 'VARCHAR'" in source_yaml


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
