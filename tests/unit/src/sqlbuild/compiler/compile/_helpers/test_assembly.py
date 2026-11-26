from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, cast

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledResourceType,
    DiscoveredProjectInputs,
    SqlHookEntry,
    assemble_project,
    build_compile_inputs,
    discover_project_inputs,
    resolve_loader_defaults,
)
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.compiler.compile.types import (
    AttachedAuditTargetKind,
    CompiledProjectView,
    CompiledSeedView,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    AssembleCompiledProjectEffectiveTargetTestCase,
    AssembleCompiledProjectTestCase,
    AssembleSqlHookValidationTestCase,
    AttachedAuditNameTestCase,
    LoadAssemblyTestCase,
    PreservedSeedTargetTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
    SQLSERVER_COMPILE_ADAPTER_CONTEXT,
    compiled_sql_test_expected_model_names,
    compiled_sql_test_tested_resource_names,
)
from tests.unit.src.sqlbuild.compiler.compile._test_helpers import (
    base_repo_files,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

_SHOP_LOAD_KEY: CompiledObjectKey = CompiledObjectKey(
    resource_type=CompiledResourceType.LOAD, name="shop"
)
_LOAD_PROJECT: str = (
    'name = "demo"\nadapter = "duckdb"\n\n'
    '[connections.local]\ndatabase = "demo.duckdb"\n\n'
    '[connections.shop_db]\nkind = "postgres"\nhost = "db"\nuser = "reader"\n'
    'database = "shop"\npassword = "${ENV:SHOP_PASSWORD_NOT_SET}"\n\n'
    '[targets.dev]\nconnection = "local"\nschema = "dev"\n'
)
_LOAD_SOURCES: str = """load:
  name: shop
  kind: postgres
  connection: shop_db
  include: [public.orders, public.customers]
sources:
  - name: shop__orders
    description: Orders
    columns:
      - name: id
        type: INTEGER
  - name: shop__customers
    description: Customers
  - name: payments
    description: Payments
"""
_LOAD_MODEL: str = """MODEL (
  materialized view,
  description "Order ids"
);

select id from __source("shop__orders")
"""


@pytest.mark.parametrize(
    "test_case",
    [
        AssembleCompiledProjectTestCase(
            description="assembles models sources seeds audits and tests into compiled project",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
default_audit_severity = "warn"

[defaults]
materialized = "table"
schema = "analytics"
""".strip()
                + "\n",
                "sql/models/staging/orders.sql": """
MODEL (description "Test model orders.",
  columns (
    order_id (type VARCHAR),
  ),
  audits [not_null (column order_id, always_run true)],
);

select * from __source("raw_orders")
""".strip()
                + "\n",
                "sql/sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    schema: public
    table: orders
""".strip()
                + "\n",
                "sql/seeds/schema.yml": """
seeds:
  - name: country_codes
    description: Test seed country_codes.
    columns:
      - name: code
        type: VARCHAR
""".strip()
                + "\n",
                "sql/seeds/country_codes.csv": "code\nUS\n",
                "sql/models/staging/_sqlbuild/_audits/generic/not_null.sql": "AUDIT ();\n\n"
                'SELECT @column FROM __ref("@model") WHERE @column IS NULL\n',
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__ref__orders AS (SELECT 'x' AS order_id),
__source__raw_orders AS (SELECT 'x' AS order_id),
__expected__orders AS (SELECT 'x' AS order_id)
SELECT 1
""".strip()
                + "\n",
            },
            expected_model_names=("orders",),
            expected_model_deps=(
                (CompiledObjectKey(resource_type=CompiledResourceType.SOURCE, name="raw_orders"),),
            ),
            expected_model_target_names=("orders",),
            expected_model_target_schemas=("analytics",),
            expected_source_names=("raw_orders",),
            expected_source_databases=(None,),
            expected_source_schemas=("public",),
            expected_seed_names=("country_codes",),
            expected_seed_target_schemas=("analytics",),
            expected_seed_target_databases=(None,),
            expected_seed_target_qualified_names=(None,),
            expected_seed_logical_schemas=("analytics",),
            expected_seed_logical_databases=(None,),
            expected_audit_names=("not_null",),
            expected_audit_scope_deps=(
                (CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),),
            ),
            expected_audit_attached_target_kinds=(AttachedAuditTargetKind.MODEL,),
            expected_audit_always_runs=(True,),
            expected_test_names=("orders_test",),
            expected_test_scope_deps=(
                (CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),),
            ),
            expected_test_expected_model_names=(("orders",),),
            expected_model_macro_deps=((),),
            expected_test_modes=("model",),
            expected_tested_macro_names=((),),
        ),
        AssembleCompiledProjectTestCase(
            description="assembles standalone audit always_run header option",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS order_id\n",
                "sql/seeds/schema.yml": """
seeds:
  - name: order_codes
    description: Test seed order_codes.
    columns:
      - name: order_id
        type: INTEGER
""".strip()
                + "\n",
                "sql/seeds/order_codes.csv": "order_id\n1\n",
                "sql/audits/singular/orders.sql": """
AUDIT (always_run true);

SELECT order_id FROM __ref("orders")
WHERE order_id NOT IN (SELECT order_id FROM __seed("order_codes"))
""".strip()
                + "\n",
            },
            expected_model_names=("orders",),
            expected_model_deps=((),),
            expected_model_target_names=("orders",),
            expected_model_target_schemas=(None,),
            expected_source_names=(),
            expected_seed_names=("order_codes",),
            expected_seed_target_schemas=(None,),
            expected_seed_target_databases=(None,),
            expected_seed_target_qualified_names=(None,),
            expected_seed_logical_schemas=(None,),
            expected_seed_logical_databases=(None,),
            expected_audit_names=("orders",),
            expected_audit_scope_deps=(
                (
                    CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
                    CompiledObjectKey(resource_type=CompiledResourceType.SEED, name="order_codes"),
                ),
            ),
            expected_audit_attached_target_kinds=(None,),
            expected_audit_always_runs=(True,),
            expected_test_names=(),
            expected_test_scope_deps=(),
            expected_test_expected_model_names=(),
            expected_model_macro_deps=((),),
            expected_test_modes=(),
            expected_tested_macro_names=(),
        ),
        AssembleCompiledProjectTestCase(
            description="assembles seed locations using local environment database templates",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[vars]
project = "reporting"

[defaults]
schema = "analytics"

[targets]

[targets.dev]
database = "${ENV:TARGET_DB}_${project}"
schema = "seeds_${project}"
""".strip()
                + "\n",
                "sqlbuild_local.toml": """
[targets]

[targets.dev]
database = "${ENV:LOCAL_TARGET_DB}_${project}"
""".strip()
                + "\n",
                "sql/seeds/schema.yml": """
seeds:
  - name: country_codes
    description: Test seed country_codes.
    columns:
      - name: code
        type: VARCHAR
""".strip()
                + "\n",
                "sql/seeds/country_codes.csv": "code\nUS\n",
            },
            expected_model_names=(),
            expected_model_deps=(),
            expected_model_target_names=(),
            expected_model_target_schemas=(),
            expected_source_names=(),
            expected_seed_names=("country_codes",),
            expected_seed_target_schemas=("seeds_reporting",),
            expected_seed_target_databases=("local_reportingdb_reporting",),
            expected_seed_target_qualified_names=(None,),
            expected_seed_logical_schemas=("analytics",),
            expected_seed_logical_databases=(None,),
            expected_audit_names=(),
            expected_audit_scope_deps=(),
            expected_test_names=(),
            expected_test_scope_deps=(),
            expected_test_expected_model_names=(),
            expected_model_macro_deps=(),
            expected_test_modes=(),
            expected_tested_macro_names=(),
        ),
        AssembleCompiledProjectTestCase(
            description="assembles seed locations using seed declaration templates",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[vars]
seed_schema_suffix = "lookups"

[defaults]
database = "default_db"
schema = "default_schema"

[targets]

[targets.dev]
database = "env_db"
schema = "env_schema"
""".strip()
                + "\n",
                "sql/seeds/lookups.yml": """
seeds:
  - name: country_codes
    description: Test seed country_codes.
    database: "${if(ENV:USE_SEED_DB, 'seed_db', CTX:destination.database)}"
    schema: "${CTX:destination.schema}_${seed_schema_suffix}"
    columns:
      - name: code
        type: VARCHAR
""".strip()
                + "\n",
                "sql/seeds/country_codes.csv": "code\nUS\n",
            },
            expected_model_names=(),
            expected_model_deps=(),
            expected_model_target_names=(),
            expected_model_target_schemas=(),
            expected_source_names=(),
            expected_seed_names=("country_codes",),
            expected_seed_target_schemas=("env_schema",),
            expected_seed_target_databases=("env_db",),
            expected_seed_target_qualified_names=(None,),
            expected_seed_logical_schemas=("default_schema_lookups",),
            expected_seed_logical_databases=("seed_db",),
            expected_audit_names=(),
            expected_audit_scope_deps=(),
            expected_test_names=(),
            expected_test_scope_deps=(),
            expected_test_expected_model_names=(),
            expected_model_macro_deps=(),
            expected_test_modes=(),
            expected_tested_macro_names=(),
        ),
        AssembleCompiledProjectTestCase(
            description="assembles macro test scope from models using inferred macro deps",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/macros/python/status.py": (
                    'def normalize_status(value):\n    return f"LOWER(TRIM({value}))"\n'
                ),
                "sql/models/orders.sql": (
                    "MODEL (description 'Test model orders.');\n\nSELECT @py.normalize_status(\"'  PAID  '\") AS status\n"
                ),
                "sql/models/customers.sql": "MODEL (description 'Test model customers.');\n\nSELECT 'active' AS status\n",
                "tests/unit/test_normalize_status.sql": """
TEST (mode macro, name "normalizes_status");

WITH
input_values AS (SELECT '  PAID  ' AS raw_status),
__macro_actual__ AS (
  SELECT @py.normalize_status("raw_status") AS status FROM input_values
),
__macro_expected__ AS (SELECT 'paid' AS status)
SELECT 1
""".strip()
                + "\n",
            },
            expected_model_names=("customers", "orders"),
            expected_model_deps=((), ()),
            expected_model_target_names=("customers", "orders"),
            expected_model_target_schemas=(None, None),
            expected_source_names=(),
            expected_seed_names=(),
            expected_audit_names=(),
            expected_audit_scope_deps=(),
            expected_test_names=("normalizes_status",),
            expected_test_scope_deps=(
                (CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),),
            ),
            expected_test_expected_model_names=((),),
            expected_model_macro_deps=((), ("normalize_status",)),
            expected_test_modes=("macro",),
            expected_tested_macro_names=(("normalize_status",),),
        ),
        AssembleCompiledProjectTestCase(
            description="applies environment namespace to managed source loader targets",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "bigquery"
default_target = "dev"

[targets.dev]
database = "project_id"
schema = "analytics_dev"
loader_schema = "raw_${suffix}"

[targets.dev.vars]
suffix = "dev"
""".strip()
                + "\n",
                "sql/sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
""".strip()
                + "\n",
                **node_module_files(
                    "python/nodes/loaders/raw_orders.py",
                    """
from sqlbuild.loaders import loader

@loader(write_strategy='replace')
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    return [{"id": 1}]
""".strip()
                    + "\n",
                ),
            },
            expected_model_names=(),
            expected_model_deps=(),
            expected_model_target_names=(),
            expected_model_target_schemas=(),
            expected_source_names=("raw_orders",),
            expected_source_databases=("project_id",),
            expected_source_schemas=("raw_dev",),
            expected_seed_names=(),
            expected_audit_names=(),
            expected_audit_scope_deps=(),
            expected_test_names=(),
            expected_test_scope_deps=(),
            expected_test_expected_model_names=(),
        ),
        AssembleCompiledProjectTestCase(
            description="assembles udf test scope from models using inferred udf deps",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/functions/sql/format_cents.sql": """
FUNCTION (description "Test function format_cents.",
  arguments (amount_cents INTEGER),
  returns VARCHAR,
);

'$' || CAST(amount_cents / 100 AS VARCHAR)
""".strip()
                + "\n",
                "sql/models/orders.sql": (
                    'MODEL (description "Test model orders.");\n\nSELECT __udf("format_cents")(1250) AS formatted\n'
                ),
                "sql/models/customers.sql": "MODEL (description 'Test model customers.');\n\nSELECT 'active' AS status\n",
                "tests/unit/test_format_cents.sql": """
TEST (mode udf, name "formats_cents");

WITH
input_values AS (SELECT 1250 AS amount_cents),
__udf_actual__ AS (
  SELECT __udf("format_cents")(amount_cents) AS formatted FROM input_values
),
__udf_expected__ AS (SELECT '$12.50' AS formatted)
SELECT 1
""".strip()
                + "\n",
            },
            expected_model_names=("customers", "orders"),
            expected_model_deps=(
                (),
                (CompiledObjectKey(resource_type=CompiledResourceType.UDF, name="format_cents"),),
            ),
            expected_model_target_names=("customers", "orders"),
            expected_model_target_schemas=(None, None),
            expected_source_names=(),
            expected_seed_names=(),
            expected_audit_names=(),
            expected_audit_scope_deps=(),
            expected_test_names=("formats_cents",),
            expected_test_scope_deps=(
                (CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),),
            ),
            expected_test_expected_model_names=((),),
            expected_model_macro_deps=((), ()),
            expected_test_modes=("udf",),
            expected_tested_macro_names=(("format_cents",),),
        ),
        AssembleCompiledProjectTestCase(
            description="assembles table function test scope from inferred table function deps",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/functions/sql/customer_orders.sql": """
FUNCTION (description "Test function customer_orders.",
  arguments (p_customer_id INTEGER),
  returns table (
    customer_id INTEGER,
    order_id INTEGER
  )
);

SELECT p_customer_id AS customer_id, 1 AS order_id
""".strip()
                + "\n",
                "sql/models/customer_order_summary.sql": (
                    "MODEL (description 'Test model customer_order_summary.');\n\nSELECT customer_id, order_id "
                    'FROM __table_fn("customer_orders")(42)\n'
                ),
                "tests/unit/test_customer_orders.sql": """
TEST (mode table_fn, name "returns_customer_orders");

WITH
__table_fn_actual__ AS (
  SELECT customer_id, order_id FROM __table_fn("customer_orders")(42)
),
__table_fn_expected__ AS (SELECT 42 AS customer_id, 1 AS order_id)
SELECT 1
""".strip()
                + "\n",
            },
            expected_model_names=("customer_order_summary",),
            expected_model_deps=(
                (
                    CompiledObjectKey(
                        resource_type=CompiledResourceType.TABLE_FN,
                        name="customer_orders",
                    ),
                ),
            ),
            expected_model_target_names=("customer_order_summary",),
            expected_model_target_schemas=(None,),
            expected_source_names=(),
            expected_seed_names=(),
            expected_audit_names=(),
            expected_audit_scope_deps=(),
            expected_test_names=("returns_customer_orders",),
            expected_test_scope_deps=(
                (
                    CompiledObjectKey(
                        resource_type=CompiledResourceType.TABLE_FN, name="customer_orders"
                    ),
                ),
            ),
            expected_test_expected_model_names=((),),
            expected_model_macro_deps=((),),
            expected_test_modes=("table_fn",),
            expected_tested_macro_names=(("customer_orders",),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compile_inputs_when_assembling_compiled_project_then_returns_expected_resources(
    test_case: AssembleCompiledProjectTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    monkeypatch.setenv("TARGET_DB", "projectdb")
    monkeypatch.setenv("LOCAL_TARGET_DB", "local_reportingdb")
    monkeypatch.setenv("USE_SEED_DB", "1")
    write_repo_files(tmp_path, test_case.repo_files)
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run_id",
    )
    compiled: CompiledProjectView = assemble_project(inputs=compile_inputs)

    assert tuple(m.name for m in compiled.models) == test_case.expected_model_names
    assert tuple(m.deps for m in compiled.models) == test_case.expected_model_deps
    assert tuple(m.macro_deps for m in compiled.models) == test_case.expected_model_macro_deps
    assert (
        tuple(m.destination.name for m in compiled.models) == test_case.expected_model_target_names
    )
    assert (
        tuple(m.destination.schema for m in compiled.models)
        == test_case.expected_model_target_schemas
    )
    assert tuple(s.name for s in compiled.sources) == test_case.expected_source_names
    assert (
        tuple(s.source_entry.database for s in compiled.sources)
        == test_case.expected_source_databases
    )
    assert (
        tuple(s.source_entry.schema for s in compiled.sources) == test_case.expected_source_schemas
    )
    assert tuple(s.name for s in compiled.seeds) == test_case.expected_seed_names
    assert (
        tuple(s.destination.schema for s in compiled.seeds)
        == test_case.expected_seed_target_schemas
    )
    assert (
        tuple(s.destination.database for s in compiled.seeds)
        == test_case.expected_seed_target_databases
    )
    assert (
        tuple(s.destination.qualified_name for s in compiled.seeds)
        == test_case.expected_seed_target_qualified_names
    )
    actual_seed_logical_schemas: tuple[str | None, ...] = tuple(
        s.destination.logical_schema for s in compiled.seeds
    )
    assert actual_seed_logical_schemas == test_case.expected_seed_logical_schemas
    actual_seed_logical_databases: tuple[str | None, ...] = tuple(
        s.destination.logical_database for s in compiled.seeds
    )
    assert actual_seed_logical_databases == test_case.expected_seed_logical_databases
    assert tuple(a.name for a in compiled.audits) == test_case.expected_audit_names
    assert tuple(a.scope_deps for a in compiled.audits) == test_case.expected_audit_scope_deps
    assert (
        tuple(a.attached_target_kind for a in compiled.audits)
        == test_case.expected_audit_attached_target_kinds
    )
    assert tuple(a.always_run for a in compiled.audits) == test_case.expected_audit_always_runs
    assert tuple(t.name for t in compiled.sql_tests) == test_case.expected_test_names
    assert tuple(t.scope_deps for t in compiled.sql_tests) == test_case.expected_test_scope_deps
    assert tuple(t.mode.value for t in compiled.sql_tests) == test_case.expected_test_modes
    assert (
        tuple(compiled_sql_test_tested_resource_names(t) for t in compiled.sql_tests)
        == test_case.expected_tested_macro_names
    )
    assert (
        tuple(compiled_sql_test_expected_model_names(t) for t in compiled.sql_tests)
        == test_case.expected_test_expected_model_names
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AttachedAuditNameTestCase(
            description="named instances of one generic audit",
            expected_names=("positive_revenue", "bounded_revenue"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_named_generic_audit_instances_when_assembling_then_each_name_is_preserved(
    test_case: AttachedAuditNameTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        base_repo_files()
        | {
            "sql/models/orders.sql": """
MODEL (description "Test model orders.",
  audits [
    expression_is_true (name "positive_revenue", expression "revenue > 0"),
    expression_is_true (name "bounded_revenue", expression "revenue < 1000"),
  ],
);

SELECT 1 AS revenue
""".strip()
            + "\n",
            "sql/audits/generic/expression_is_true.sql": """
AUDIT ();

SELECT * FROM @relation WHERE NOT (@expression)
""".strip()
            + "\n",
        },
    )
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run_id",
    )

    compiled: CompiledProjectView = assemble_project(inputs=compile_inputs)

    assert tuple(audit.name for audit in compiled.audits) == test_case.expected_names


@pytest.mark.parametrize(
    "test_case",
    (
        PreservedSeedTargetTestCase(
            description="resolves seed namespace precedence across defaults declarations and targets",
            expected_declared_location=("declared_seed_db", "declared_seed_schema"),
            expected_defaulted_location=("default_seed_db", "default_seed_schema"),
            expected_literal_target_location=("physical_db", "physical_schema"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_preserved_target_when_assembling_seed_then_declaration_remains_physical(
    test_case: PreservedSeedTargetTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        base_repo_files()
        | {
            "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[defaults]
database = "default_db"
schema = "default_schema"
seed_database = "default_seed_db"
seed_schema = "default_seed_schema"

[targets.dev]
database = "preserve"
schema = "preserve"

[targets.physical]
database = "physical_db"
schema = "physical_schema"
""".strip()
            + "\n",
            "sql/seeds/schema.yml": """
seeds:
  - name: country_codes
    description: Test seed country_codes.
    database: declared_seed_db
    schema: declared_seed_schema
    columns:
      - name: code
        type: VARCHAR
  - name: currencies
    description: Test seed currencies.
    columns:
      - name: code
        type: VARCHAR
""".strip()
            + "\n",
            "sql/seeds/country_codes.csv": "code\nUS\n",
            "sql/seeds/currencies.csv": "code\nUSD\n",
        },
    )
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )

    seeds: dict[str, CompiledSeedView] = {
        seed.name: seed for seed in assemble_project(inputs=compile_inputs).seeds
    }

    assert (
        seeds["country_codes"].destination.database,
        seeds["country_codes"].destination.schema,
    ) == test_case.expected_declared_location
    assert (
        seeds["country_codes"].destination.logical_database,
        seeds["country_codes"].destination.logical_schema,
    ) == test_case.expected_declared_location
    assert (
        seeds["currencies"].destination.database,
        seeds["currencies"].destination.schema,
        seeds["currencies"].destination.logical_database,
        seeds["currencies"].destination.logical_schema,
    ) == test_case.expected_defaulted_location * 2

    physical_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        selected_target="physical",
    )
    physical_seeds: tuple[CompiledSeedView, ...] = assemble_project(inputs=physical_inputs).seeds
    assert {(seed.destination.database, seed.destination.schema) for seed in physical_seeds} == {
        test_case.expected_literal_target_location
    }


@pytest.mark.parametrize(
    "test_case",
    (
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="a target never takes its connection's location on duckdb",
            project_toml="""
name = "demo"
adapter = "duckdb"
default_target = "dev"

[targets.dev.connection]
database = "connection_db"
schema = "connection_schema"
""".strip()
            + "\n",
            expected_effective_target_database=None,
            expected_effective_target_schema=None,
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="a target never takes its connection's location on postgres",
            project_toml="""
name = "demo"
adapter = "postgres"
default_target = "dev"

[targets.dev.connection]
database = "connection_db"
schema = "connection_schema"
""".strip()
            + "\n",
            expected_effective_target_database=None,
            expected_effective_target_schema=None,
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="prefers target location over connection fallback",
            project_toml="""
name = "demo"
adapter = "duckdb"
default_target = "dev"

[targets.dev]
database = "target_db"
schema = "target_schema"

[targets.dev.connection]
database = "connection_db"
schema = "connection_schema"
""".strip()
            + "\n",
            expected_effective_target_database="target_db",
            expected_effective_target_schema="target_schema",
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="prefers defaults location over connection fallback",
            project_toml="""
name = "demo"
adapter = "postgres"
default_target = "dev"

[defaults]
database = "defaults_db"
schema = "defaults_schema"

[targets.dev.connection]
database = "connection_db"
schema = "connection_schema"
""".strip()
            + "\n",
            expected_effective_target_database="defaults_db",
            expected_effective_target_schema="defaults_schema",
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="never falls back to the connection location on snowflake",
            project_toml="""
name = "demo"
adapter = "snowflake"
default_target = "dev"

[targets.dev.connection]
database = "connection_db"
schema = "connection_schema"
""".strip()
            + "\n",
            expected_effective_target_database=None,
            expected_effective_target_schema=None,
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="an inline connection is the implicit default target on snowflake",
            project_toml="""
name = "demo"
adapter = "snowflake"

[connection]
database = "CONN_DB"
schema = "CONN_SCHEMA"
""".strip()
            + "\n",
            expected_effective_target_database="CONN_DB",
            expected_effective_target_schema="CONN_SCHEMA",
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="an inline connection schema fills in beside a defaults database",
            project_toml="""
name = "demo"
adapter = "bigquery"

[connection]
project = "example-project"
schema = "orders"

[defaults]
database = "example-project"
""".strip()
            + "\n",
            expected_effective_target_database="example-project",
            expected_effective_target_schema="orders",
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="a preserved target schema reaches models and loaders alike",
            project_toml="""
name = "demo"
adapter = "duckdb"
default_target = "dev"

[defaults]
schema = "analytics"

[targets.dev]
schema = "preserve"
""".strip()
            + "\n",
            expected_effective_target_database=None,
            expected_effective_target_schema="preserve",
        ),
        AssembleCompiledProjectEffectiveTargetTestCase(
            description="a top-level connection beside named targets is not used",
            project_toml="""
name = "demo"
adapter = "snowflake"
default_target = "dev"

[connection]
database = "CONN_DB"
schema = "CONN_SCHEMA"

[targets.dev]
database = "TARGET_DB"
""".strip()
            + "\n",
            expected_effective_target_database="TARGET_DB",
            expected_effective_target_schema=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_connection_location_when_assembling_project_then_models_and_loaders_agree(
    test_case: AssembleCompiledProjectEffectiveTargetTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        base_repo_files() | {"sqlbuild_project.toml": test_case.project_toml},
    )
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run_id",
    )

    compiled: CompiledProjectView = assemble_project(inputs=compile_inputs)
    loader_location: tuple[str | None, str | None] = resolve_loader_defaults(
        discovered_inputs=discovered,
        selected_target=compiled.effective_target_name,
        target_config=compile_inputs.effective_target,
    )

    expected_location: tuple[str | None, str | None] = (
        test_case.expected_effective_target_database,
        test_case.expected_effective_target_schema,
    )
    assert (
        compiled.effective_target_database,
        compiled.effective_target_schema,
    ) == expected_location, test_case.description
    assert loader_location == expected_location, f"{test_case.description}: loaders"


@pytest.mark.parametrize(
    "test_case",
    (
        AssembleSqlHookValidationTestCase(
            description="validates named and inline payloads with adapter dialect",
            model_sql=(
                'MODEL (description "Test model.", pre_hooks [sql("vendor_hook")], '
                'post_hooks [inline_sql("SELECT [other] FROM [orders]; '
                'SELECT [other] FROM [orders]")]);\n\nSELECT 1 AS id\n'
            ),
            named_hook_sql=(
                "HOOK (description 'Test hook.');\n\nSELECT [value] FROM [orders]; SELECT [value] FROM [orders]\n"
            ),
            dialect="tsql",
            expected_hook_statements=(
                "SELECT [value] FROM [orders]; SELECT [value] FROM [orders]",
                "SELECT [other] FROM [orders]; SELECT [other] FROM [orders]",
            ),
        ),
        AssembleSqlHookValidationTestCase(
            description="skips named and inline Polyglot validation for model opt out",
            model_sql=(
                'MODEL (description "Test model.", sql_validation false, pre_hooks [sql("vendor_hook")], '
                'post_hooks [inline_sql("DBCC CHECKIDENT (orders, RESEED, 0)")]);'
                "\n\nSELECT 1 AS id\n"
            ),
            named_hook_sql="HOOK (description 'Test hook.');\n\nDBCC CHECKIDENT ('orders', RESEED, 0)\n",
            dialect="tsql",
            expected_hook_statements=(
                "DBCC CHECKIDENT ('orders', RESEED, 0)",
                "DBCC CHECKIDENT (orders, RESEED, 0)",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_hooks_when_assembling_then_applies_adapter_aware_validation(
    test_case: AssembleSqlHookValidationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "demo"\nadapter = "sqlserver"\n',
            "sql/models/orders.sql": test_case.model_sql,
            "sql/hooks/sql/vendor_hook.sql": test_case.named_hook_sql,
        },
    )
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=SQLSERVER_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run_id",
    )

    compiled: CompiledProjectView = assemble_project(
        inputs=compile_inputs,
        inference_profile=ExpressionInferenceProfile(
            sql_analysis_dialect=test_case.dialect,
        ),
    )

    pre_hooks: list[SqlHookEntry] = cast(
        list[SqlHookEntry],
        compiled.models[0].config.values["pre_hooks"],
    )
    post_hooks: list[SqlHookEntry] = cast(
        list[SqlHookEntry],
        compiled.models[0].config.values["post_hooks"],
    )
    assert (pre_hooks[0].statement, post_hooks[0].statement) == test_case.expected_hook_statements


@pytest.mark.parametrize(
    "test_case",
    [
        LoadAssemblyTestCase(
            description="the load is a node its sources depend on, and load:shop+ reaches models",
            repo_files={
                "sqlbuild_project.toml": _LOAD_PROJECT,
                "sql/sources/shop.yml": _LOAD_SOURCES,
                "sql/models/order_ids.sql": _LOAD_MODEL,
            },
            selector="load:shop+",
            expected_loads=(("shop", ("shop__orders", "shop__customers")),),
            expected_source_deps=(
                ("shop__orders", (_SHOP_LOAD_KEY,)),
                ("shop__customers", (_SHOP_LOAD_KEY,)),
                ("payments", ()),
            ),
            expected_selected=(
                ("load", "shop"),
                ("model", "order_ids"),
                ("source", "shop__customers"),
                ("source", "shop__orders"),
            ),
        ),
        LoadAssemblyTestCase(
            description="kind:load selects every load and nothing else",
            repo_files={
                "sqlbuild_project.toml": _LOAD_PROJECT,
                "sql/sources/shop.yml": _LOAD_SOURCES,
                "sql/models/order_ids.sql": _LOAD_MODEL,
            },
            selector="kind:load",
            expected_loads=(("shop", ("shop__orders", "shop__customers")),),
            expected_source_deps=(
                ("shop__orders", (_SHOP_LOAD_KEY,)),
                ("shop__customers", (_SHOP_LOAD_KEY,)),
                ("payments", ()),
            ),
            expected_selected=(("load", "shop"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_load_when_assembling_then_it_is_a_graph_node_its_sources_depend_on(
    test_case: LoadAssemblyTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run_id",
    )

    compiled: CompiledProjectView = assemble_project(inputs=compile_inputs)

    loads: tuple[tuple[str, tuple[str, ...]], ...] = tuple(
        (load.name, load.sources) for load in compiled.loads
    )
    assert loads == test_case.expected_loads, test_case.description
    source_deps: tuple[tuple[str, tuple[CompiledObjectKey, ...]], ...] = tuple(
        (source.name, source.deps) for source in compiled.sources
    )
    assert source_deps == test_case.expected_source_deps, test_case.description
    keys, failure = compiled.lineage_graph.resolve([test_case.selector], [])
    assert failure is None, test_case.description
    assert tuple(sorted(keys or ())) == test_case.expected_selected, test_case.description
