from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, cast

import pytest

from sqlbuild._native import (
    DeclarationParseError,
    DiscoveredProjectInputs,
    ResourceIdentityError,
    SqlHookEntry,
    assemble_project,
    build_compile_inputs,
    build_model_version_identity_metadata_json,
    compute_query_hash,
    discover_project_inputs,
)
from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.types import CompiledModelView, CompiledProjectView
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    CompileDeclarationsErrorTestCase,
    CompileDeclarationsTestCase,
    CompileTypedConstantsTestCase,
    DeclarationFingerprintTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_ARRAY_COMPILE_ADAPTER_CONTEXT,
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
    compile_first_model,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView, ModelInput

_PROJECT_FILE: str = """
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = false
sql_validation = false
"""


@pytest.mark.parametrize(
    "test_case",
    [
        CompileDeclarationsTestCase(
            description="public and private declarations",
            expected_query_sql="""SELECT
  'DELIVERY' AS fulfillment_method,
  3 AS priority,
  'OPEN' AS state,
  7 + 2 AS threshold,
  'O''Brien' AS source_name""",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_public_and_private_declarations_when_compiling_then_resolves_scopes_and_contracts(
    test_case: CompileDeclarationsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/models/_enums/market.sql": """
ENUM (name fulfillment_method, members [DELIVERY, PICKUP]);
ENUM (name priority, members (LOW 1, HIGH 3));
""",
            "sql/constants/market/thresholds.sql": """
CONSTANT (name min_items, value 7);
""",
            "sql/models/_constants/thresholds.sql": """
CONSTANT (name source_name, value "O'Brien");
""",
            "sql/models/orders.sql": """
MODEL (description "Test model orders.",
  contract enforced,
  pre_hooks [inline_sql('SELECT @const("min_items") + @const("_offset")')],
  enums (
    _state [OPEN, CLOSED],
  ),
  constants (
    _offset 2,
  ),
  columns (
    fulfillment_method (type fulfillment_method),
    priority (type priority),
    state (type _state),
    threshold (type INTEGER),
    source_name (type VARCHAR),
  ),
);

SELECT
  @enum("fulfillment_method").DELIVERY AS fulfillment_method,
  @enum("priority").HIGH AS priority,
  @enum("_state").OPEN AS state,
  @const("min_items") + @const("_offset") AS threshold,
  @const("source_name") AS source_name
""",
            "sql/functions/sql/add_threshold.sql": """
FUNCTION (description "Test function add_threshold.",
  arguments (value INTEGER),
  returns INTEGER,
);

value + @const("min_items")
""",
            "sql/sources/inline.yml": """
sources:
  - name: inline_values
    description: Test source inline_values.
    expression: |
      SELECT @const("min_items") AS value
""",
            "tests/unit/orders_test.sql": """
TEST ();

WITH __ref__orders AS (
  SELECT @enum("fulfillment_method").DELIVERY AS fulfillment_method
),
__expected__orders AS (
  SELECT @enum("fulfillment_method").DELIVERY AS fulfillment_method
)
SELECT 1
""",
            "tests/scenarios/orders_scenario.sql": """
SCENARIO (description "Test scenario orders.");

WITH __ref__orders AS (
  SELECT @const("min_items") AS threshold
),
__expected__orders AS (
  SELECT @const("min_items") AS threshold
)
SELECT 1
""",
            "sql/audits/singular/orders.sql": """
AUDIT ();

SELECT * FROM __ref("orders") WHERE threshold < @const("min_items")
  AND threshold NOT IN (SELECT value FROM __source("inline_values"))
""",
        },
    )
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run",
    )

    model_input: ModelInput = compile_inputs.model_inputs[0]
    assert model_input.schema_entry is not None
    assert model_input.query_sql == test_case.expected_query_sql
    assert tuple(compile_inputs.public_enums) == ("fulfillment_method", "priority")
    assert tuple(compile_inputs.public_constants) == ("min_items", "source_name")
    assert tuple(declaration.name for declaration in model_input.enum_declarations) == ("_state",)
    assert tuple(declaration.name for declaration in model_input.constant_declarations) == (
        "_offset",
    )
    assert tuple(model_input.enum_columns) == ("fulfillment_method", "priority", "state")
    assert model_input.config.values["pre_hooks"] == [SqlHookEntry(statement="SELECT 7 + 2")]
    assert tuple(column.type for column in model_input.schema_entry.columns) == (
        "VARCHAR",
        "INTEGER",
        "VARCHAR",
        "INTEGER",
        "VARCHAR",
    )
    actual_column_audits: list[tuple[str, ...]] = []
    for column in model_input.schema_entry.columns:
        actual_column_audits.append(tuple(audit.definition_name for audit in column.audits))
    assert tuple(actual_column_audits) == (
        ("accepted_values",),
        ("accepted_values",),
        ("accepted_values",),
        (),
        (),
    )
    assert tuple(audit.sql_body for audit in compile_inputs.audit_inputs) == (
        'SELECT * FROM __ref("orders") WHERE threshold < 7\n'
        '  AND threshold NOT IN (SELECT value FROM __source("inline_values"))',
        'SELECT fulfillment_method\nFROM __ref("orders")\nWHERE fulfillment_method IS NOT NULL\n'
        "  AND fulfillment_method NOT IN ('DELIVERY', 'PICKUP')",
        'SELECT priority\nFROM __ref("orders")\nWHERE priority IS NOT NULL\n'
        "  AND priority NOT IN (1, 3)",
        'SELECT state\nFROM __ref("orders")\nWHERE state IS NOT NULL\n'
        "  AND state NOT IN ('OPEN', 'CLOSED')",
    )
    assert tuple(function.body_sql for function in compile_inputs.sql_function_inputs) == (
        "value + 7",
    )
    assert tuple(source.source_entry.expression for source in compile_inputs.source_inputs) == (
        "SELECT 7 AS value\n",
    )
    assert tuple(test.sql_body for test in compile_inputs.test_inputs) == (
        "WITH __ref__orders AS (\n  SELECT 'DELIVERY' AS fulfillment_method\n),\n"
        "__expected__orders AS (\n  SELECT 'DELIVERY' AS fulfillment_method\n)\nSELECT 1",
    )
    assert tuple(scenario.sql_body for scenario in compile_inputs.scenario_inputs) == (
        "WITH __ref__orders AS (\n  SELECT 7 AS threshold\n),\n"
        "__expected__orders AS (\n  SELECT 7 AS threshold\n)\nSELECT 1",
    )
    compiled_project: CompiledProjectView = assemble_project(
        inputs=compile_inputs,
        skip_column_inference=True,
    )
    compiled_model: CompiledModelView = compiled_project.models[0]
    assert tuple(compiled_project.public_enums) == ("fulfillment_method", "priority")
    assert tuple(compiled_project.public_constants) == ("min_items", "source_name")
    assert tuple(compiled_model.enum_columns) == ("fulfillment_method", "priority", "state")
    identity_metadata: dict[str, object] = json.loads(
        build_model_version_identity_metadata_json(model=compiled_model)
    )
    assert identity_metadata["execution_signature"] == {
        "pre_hooks": [{"type": "sql", "statement": "SELECT 7 + 2"}],
        "contract": {
            "enforced": True,
            "columns": [
                {
                    "name": "fulfillment_method",
                    "type": "VARCHAR",
                    "nullable": None,
                    "enum": {
                        "name": "fulfillment_method",
                        "members": [
                            {"name": "DELIVERY", "value": "DELIVERY"},
                            {"name": "PICKUP", "value": "PICKUP"},
                        ],
                    },
                },
                {
                    "name": "priority",
                    "type": "INTEGER",
                    "nullable": None,
                    "enum": {
                        "name": "priority",
                        "members": [
                            {"name": "LOW", "value": 1},
                            {"name": "HIGH", "value": 3},
                        ],
                    },
                },
                {
                    "name": "state",
                    "type": "VARCHAR",
                    "nullable": None,
                    "enum": {
                        "name": "_state",
                        "members": [
                            {"name": "OPEN", "value": "OPEN"},
                            {"name": "CLOSED", "value": "CLOSED"},
                        ],
                    },
                },
                {"name": "threshold", "type": "INTEGER", "nullable": None},
                {"name": "source_name", "type": "VARCHAR", "nullable": None},
            ],
        },
    }


@pytest.mark.parametrize(
    "test_case",
    [
        CompileTypedConstantsTestCase(
            description="typed constants expand through every authored SQL surface",
            expected_model_sql=(
                "SELECT TRUE AS enabled, 0.75 AS ratio,\n"
                "  2.4700 AS rate, NULL AS missing,\n"
                "  ('GB', 'FR') AS countries, ('FR', 'GB') AS unique_countries,\n"
                "  ['project', 'default'] AS project_array, ['GB', 'FR'] AS country_array,\n"
                '  json(\'{"FR":"France","GB":"Great Britain"}\') AS labels,\n'
                "  ['local', 'values'] AS local_values"
            ),
            expected_hook_sql="SELECT 'GB' IN ('GB', 'FR')",
            expected_named_hook_sql="SELECT 'FR' IN ('GB', 'FR')",
            expected_function_sql="value IN ('GB', 'FR')",
            expected_source_sql="SELECT * FROM entries WHERE country IN ('GB', 'FR')\n",
            expected_test_fragment="SELECT 'GB' IN ('GB', 'FR') AS supported",
            expected_scenario_fragment="SELECT ['GB', 'FR'] AS countries",
            expected_audit_sql=(
                'SELECT * FROM __ref("orders") JOIN __source("supported_entries") USING (country)\n'
                "WHERE country NOT IN ('FR', 'GB')"
            ),
            expected_attached_audit_sql=(
                "SELECT * FROM __ref(\"orders\") WHERE country NOT IN ('GB', 'FR')"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_constants_when_compiling_then_all_sql_surfaces_use_adapter_rendering(
    test_case: CompileTypedConstantsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": (
                _PROJECT_FILE + '\n[constants]\ncollection_rendering = "array"\n'
            ),
            "sql/constants/typed.sql": """
CONSTANT (name enabled, value true);
CONSTANT (name ratio, value 0.75);
CONSTANT (name rate, type decimal, value "2.4700");
CONSTANT (name missing, value null);
CONSTANT (name countries, value ["GB", "FR"], render_as value_list);
CONSTANT (name unique_countries, value {"GB", "FR"}, render_as value_list);
CONSTANT (name project_array, value ["project", "default"]);
CONSTANT (name country_array, value ["GB", "FR"], render_as array);
CONSTANT (name labels, value (GB "Great Britain", FR "France"));
""",
            "sql/models/orders.sql": """
MODEL (description "Test model orders.",
  pre_hooks [inline_sql("SELECT 'GB' IN @const('countries')")],
  post_hooks [sql("typed_hook")],
  audits [typed_audit],
  constants (
    _local_values constant(value ["local", "values"], render_as array),
  ),
);

SELECT @const("enabled") AS enabled, @const("ratio") AS ratio,
  @const("rate") AS rate, @const("missing") AS missing,
  @const("countries") AS countries, @const("unique_countries") AS unique_countries,
  @const("project_array") AS project_array, @const("country_array") AS country_array,
  @const("labels") AS labels,
  @const("_local_values") AS local_values
""",
            "sql/functions/sql/is_supported.sql": """
FUNCTION (description "Test function is_supported.", arguments (value VARCHAR), returns BOOLEAN);
value IN @const("countries")
""",
            "sql/hooks/sql/typed_hook.sql": """
HOOK (description "Typed constant hook");
SELECT 'FR' IN @const("countries")
""",
            "sql/sources/inline.yml": """
sources:
  - name: supported_entries
    description: Test source supported_entries.
    expression: |
      SELECT * FROM entries WHERE country IN @const("countries")
""",
            "tests/unit/orders_test.sql": """
TEST ();
WITH __ref__orders AS (SELECT 'GB' IN @const("countries") AS supported),
__expected__orders AS (SELECT true AS supported)
SELECT 1
""",
            "tests/scenarios/orders_scenario.sql": """
SCENARIO (description "Test scenario orders.");
WITH __ref__orders AS (SELECT @const("country_array") AS countries),
__expected__orders AS (SELECT @const("country_array") AS countries)
SELECT 1
""",
            "sql/audits/singular/orders.sql": """
AUDIT ();
SELECT * FROM __ref("orders") JOIN __source("supported_entries") USING (country)
WHERE country NOT IN @const("unique_countries")
""",
            "sql/audits/generic/typed_audit.sql": """
AUDIT ();
SELECT * FROM __ref("@model") WHERE country NOT IN @const("countries")
""",
        },
    )

    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter_context=DUCKDB_ARRAY_COMPILE_ADAPTER_CONTEXT,
        run_id="typed_constants",
    )

    assert compile_inputs.model_inputs[0].query_sql == test_case.expected_model_sql
    assert compile_inputs.model_inputs[0].config.values["pre_hooks"] == [
        SqlHookEntry(statement=test_case.expected_hook_sql)
    ]
    named_hook: SqlHookEntry = cast(
        list[SqlHookEntry], compile_inputs.model_inputs[0].config.values["post_hooks"]
    )[0]
    assert named_hook.statement == test_case.expected_named_hook_sql
    assert compile_inputs.sql_function_inputs[0].body_sql == test_case.expected_function_sql
    assert compile_inputs.source_inputs[0].source_entry.expression == test_case.expected_source_sql
    assert test_case.expected_test_fragment in compile_inputs.test_inputs[0].sql_body
    assert test_case.expected_scenario_fragment in compile_inputs.scenario_inputs[0].sql_body
    assert compile_inputs.audit_inputs[0].sql_body == test_case.expected_audit_sql
    assert compile_inputs.audit_inputs[1].sql_body == test_case.expected_attached_audit_sql


@pytest.mark.parametrize(
    "test_case",
    [
        DeclarationFingerprintTestCase(
            description="constant value changes rendered query identity",
            declaration_path="sql/models/_constants/threshold.sql",
            initial_declaration="CONSTANT (name threshold, value 7);\n",
            changed_declaration="CONSTANT (name threshold, value 8);\n",
            model_sql='MODEL (description "Test model threshold.");\nSELECT @const("threshold") AS threshold\n',
            expected_query_hash_changed=True,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="list order changes rendered query identity",
            declaration_path="sql/models/_constants/countries.sql",
            initial_declaration='CONSTANT (name countries, value ["GB", "FR"]);\n',
            changed_declaration='CONSTANT (name countries, value ["FR", "GB"]);\n',
            model_sql='MODEL (description "Test model countries.");\nSELECT @const("countries") AS countries\n',
            expected_query_hash_changed=True,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="list duplicate changes rendered query identity",
            declaration_path="sql/models/_constants/countries.sql",
            initial_declaration='CONSTANT (name countries, value ["GB", "FR"]);\n',
            changed_declaration='CONSTANT (name countries, value ["GB", "FR", "FR"]);\n',
            model_sql='MODEL (description "Test model countries.");\nSELECT @const("countries") AS countries\n',
            expected_query_hash_changed=True,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="set order does not change rendered query identity",
            declaration_path="sql/models/_constants/countries.sql",
            initial_declaration='CONSTANT (name countries, value {"GB", "FR"});\n',
            changed_declaration='CONSTANT (name countries, value {"FR", "GB"});\n',
            model_sql='MODEL (description "Test model countries.");\nSELECT @const("countries") AS countries\n',
            expected_query_hash_changed=False,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="object key order does not change rendered query identity",
            declaration_path="sql/models/_constants/labels.sql",
            initial_declaration=(
                'CONSTANT (name labels, value (GB "Great Britain", FR "France"));\n'
            ),
            changed_declaration=(
                'CONSTANT (name labels, value (FR "France", GB "Great Britain"));\n'
            ),
            model_sql='MODEL (description "Test model.");\nSELECT @const("labels") AS labels\n',
            expected_query_hash_changed=False,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="set membership changes rendered query identity",
            declaration_path="sql/models/_constants/countries.sql",
            initial_declaration='CONSTANT (name countries, value {"GB", "FR"});\n',
            changed_declaration='CONSTANT (name countries, value {"GB", "HK"});\n',
            model_sql='MODEL (description "Test model countries.");\nSELECT @const("countries") AS countries\n',
            expected_query_hash_changed=True,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="object value changes rendered query identity",
            declaration_path="sql/models/_constants/labels.sql",
            initial_declaration='CONSTANT (name labels, value (GB "Great Britain"));\n',
            changed_declaration='CONSTANT (name labels, value (GB "Britain"));\n',
            model_sql='MODEL (description "Test model labels.");\nSELECT @const("labels") AS labels\n',
            expected_query_hash_changed=True,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="collection render mode changes rendered query identity",
            declaration_path="sql/models/_constants/countries.sql",
            initial_declaration='CONSTANT (name countries, value ["GB", "FR"]);\n',
            changed_declaration=(
                'CONSTANT (name countries, value ["GB", "FR"], render_as array);\n'
            ),
            model_sql='MODEL (description "Test model.");\nSELECT @const("countries") AS countries\n',
            expected_query_hash_changed=True,
            expected_metadata_changed=False,
        ),
        DeclarationFingerprintTestCase(
            description="declaration used by another model does not alter model identity",
            declaration_path="sql/models/_constants/countries.sql",
            initial_declaration='CONSTANT (name countries, value ["GB", "FR"]);\n',
            changed_declaration='CONSTANT (name countries, value ["GB", "HK"]);\n',
            model_sql="MODEL (description 'Test model countries.');\nSELECT 1 AS value\n",
            expected_query_hash_changed=False,
            expected_metadata_changed=False,
            additional_files={
                "sql/models/z_consumer.sql": (
                    'MODEL (description "Test model z_consumer.");\nSELECT @const("countries") AS countries\n'
                )
            },
        ),
        DeclarationFingerprintTestCase(
            description="enum members change enforced contract identity",
            declaration_path="sql/models/_enums/state.sql",
            initial_declaration="ENUM (name state, members [OPEN]);\n",
            changed_declaration="ENUM (name state, members [OPEN, CLOSED]);\n",
            model_sql="""
MODEL (description "Test model state.",
  contract enforced,
  columns (state (type state)),
);
SELECT 'OPEN' AS state
""",
            expected_query_hash_changed=False,
            expected_metadata_changed=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declaration_change_when_compiling_then_updates_dependent_identity(
    test_case: DeclarationFingerprintTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            test_case.declaration_path: test_case.initial_declaration,
            "sql/models/orders.sql": test_case.model_sql,
        }
        | test_case.additional_files,
    )
    initial_model: CompiledModelView = compile_first_model(project_dir=tmp_path)

    write_repo_files(
        tmp_path,
        {test_case.declaration_path: test_case.changed_declaration},
    )
    changed_model: CompiledModelView = compile_first_model(project_dir=tmp_path)

    query_hash_changed: bool = compute_query_hash(
        query_sql=initial_model.query_sql, dialect=None
    ) != compute_query_hash(query_sql=changed_model.query_sql, dialect=None)
    metadata_changed: bool = build_model_version_identity_metadata_json(
        model=initial_model
    ) != build_model_version_identity_metadata_json(model=changed_model)
    assert query_hash_changed == test_case.expected_query_hash_changed
    assert metadata_changed == test_case.expected_metadata_changed


@pytest.mark.parametrize(
    "test_case",
    [
        CompileDeclarationsErrorTestCase(
            description="unknown enum member",
            repo_files={
                "sql/enums/state.sql": "ENUM (name state, members [OPEN, CLOSED]);",
                "sql/models/orders.sql": (
                    'MODEL (description "Test model orders.");\nSELECT @enum("state").MISSING AS state\n'
                ),
            },
            expected_error_fragment="Unknown member 'MISSING' for enum 'state'",
        ),
        CompileDeclarationsErrorTestCase(
            description="lowercase enum member access",
            repo_files={
                "sql/enums/state.sql": 'ENUM (name state, members (DELIVERY "active"));',
                "sql/models/orders.sql": (
                    'MODEL (description "Test model orders.");\nSELECT @enum("state").active AS state\n'
                ),
            },
            expected_error_fragment="Unknown member 'active' for enum 'state'",
        ),
        CompileDeclarationsErrorTestCase(
            description="unknown constant",
            repo_files={
                "sql/models/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("missing") AS value\n',
            },
            expected_error_fragment="Unknown constant 'missing'",
        ),
        CompileDeclarationsErrorTestCase(
            description="foreign private constant",
            repo_files={
                "sql/models/a.sql": "MODEL (description 'Test model a.', constants (_limit 7));\nSELECT 1 AS value\n",
                "sql/models/b.sql": 'MODEL (description "Test model b.");\nSELECT @const("_limit") AS value\n',
            },
            expected_error_fragment="Constant '_limit' is known but inaccessible",
        ),
        CompileDeclarationsErrorTestCase(
            description="non-private model constant name",
            repo_files={
                "sql/models/orders.sql": "MODEL (description 'Test model orders.', constants (limit 7));\nSELECT 1 AS value\n",
            },
            expected_error_fragment=(
                "Invalid model-local constant identity 'limit'.*use snake_case '_limit'"
            ),
        ),
        CompileDeclarationsErrorTestCase(
            description="duplicate public enum name",
            repo_files={
                "sql/enums/one.sql": "ENUM (name state, members [OPEN]);",
                "sql/enums/nested/two.sql": "ENUM (name state, members [CLOSED]);",
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value\n",
            },
            expected_error_fragment="Duplicate declaration 'enum:state'",
        ),
        CompileDeclarationsErrorTestCase(
            description="nested collection cannot render as a portable value list",
            repo_files={
                "sql/constants/groups.sql": "CONSTANT (name groups, value [[1], [2]]);",
                "sql/models/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("groups") AS groups\n',
            },
            expected_error_fragment=(
                "sql/constants/groups.sql constant 'groups'.*adapter 'duckdb'.*value_list"
            ),
        ),
        CompileDeclarationsErrorTestCase(
            description="constant rendered over the SQL value size limit",
            repo_files={
                "sql/constants/big.sql": f'CONSTANT (name big, value "{"'" * 600_000}");',
                "sql/models/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("big") AS big\n',
            },
            expected_error_fragment="sql/constants/big.sql constant 'big'.*maximum is 1000000 bytes",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_declaration_usage_when_compiling_then_raises_compile_error(
    test_case: CompileDeclarationsErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.repo_files,
    )
    with pytest.raises(
        (CompileInputError, DeclarationParseError, ResourceIdentityError),
        match=test_case.expected_error_fragment,
    ):
        discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
        build_compile_inputs(
            discovered_inputs=discovered_inputs,
            adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
            run_id="test_run",
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
