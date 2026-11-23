from __future__ import annotations

import re
from collections.abc import Callable, Iterator
from dataclasses import replace
from itertools import chain
from pathlib import Path
from typing import TYPE_CHECKING, cast

import pytest

from sqlbuild._native import (
    DeclarationIdentity,
    DeclarationKind,
    DiscoveredProjectInputs,
    DiscoveredSourceFile,
    ResourceIdentity,
    ResourceKind,
    SourceEntry,
    SqlHookEntry,
    UsageKind,
    UsageRecord,
    assemble_project,
    build_compile_inputs,
    discover_project_inputs,
    scope_metadata_projection,
)
from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    ExpectedBooleanTestCase,
    ExpectedModelDeclarationGrantTestCase,
    RelationshipUsageTestCase,
    ScopedDeclarationCompileTestCase,
    ScopedDeclarationErrorTestCase,
    ScopedDeclarationSurfaceTestCase,
    ScopePlacementCompileTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
    compile_project_inputs,
    render_compile_diagnostics,
    usage_consumers,
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
        ExpectedBooleanTestCase(
            description="models sharing a folder each record their own enum usage",
            expected_result=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_models_in_one_folder_when_compiling_then_folder_visibility_is_rebound_per_model(
    test_case: ExpectedBooleanTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/models/domain/_enums/status.sql": "ENUM (name status, members [OPEN, SHIPPED]);",
            "sql/models/domain/customers.sql": (
                'MODEL (description "Test model customers.");\n'
                'SELECT @enum("status").OPEN AS status\n'
            ),
            "sql/models/domain/orders.sql": (
                'MODEL (description "Test model orders.");\n'
                'SELECT @enum("status").SHIPPED AS status\n'
            ),
        },
    )

    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)

    consumers: list[set[ResourceIdentity | DeclarationIdentity]] = [
        usage_consumers(model_input) for model_input in inputs.model_inputs
    ]
    assert (
        consumers
        == [
            {ResourceIdentity(ResourceKind.MODEL, "customers")},
            {ResourceIdentity(ResourceKind.MODEL, "orders")},
        ]
    ) is test_case.expected_result


@pytest.mark.parametrize(
    "test_case",
    (
        ScopedDeclarationCompileTestCase(
            "global declaration",
            {"sql/constants/value.sql": "CONSTANT (name value, value 1);"},
            "sql/models/domain/child/orders.sql",
            'MODEL (description "Test model orders.");\nSELECT @const("value") AS value\n',
            "SELECT 1 AS value",
        ),
        ScopedDeclarationCompileTestCase(
            "inherited declaration",
            {"sql/models/domain/constants/value.sql": "CONSTANT (name value, value 2);"},
            "sql/models/domain/child/orders.sql",
            'MODEL (description "Test model orders.");\nSELECT @const("value") AS value\n',
            "SELECT 2 AS value",
        ),
        ScopedDeclarationCompileTestCase(
            "all ancestor declarations",
            {
                "sql/models/domain/constants/parent.sql": "CONSTANT (name parent_value, value 2);",
                "sql/models/domain/child/constants/child.sql": (
                    "CONSTANT (name child_value, value 3);"
                ),
            },
            "sql/models/domain/child/grandchild/orders.sql",
            (
                'MODEL (description "Test model orders.");\nSELECT @const("parent_value") + @const("child_value") AS value\n'
            ),
            "SELECT 2 + 3 AS value",
        ),
        ScopedDeclarationCompileTestCase(
            "local declaration",
            {"sql/models/domain/_constants/value.sql": "CONSTANT (name value, value 4);"},
            "sql/models/domain/orders.sql",
            'MODEL (description "Test model orders.");\nSELECT @const("value") AS value\n',
            "SELECT 4 AS value",
        ),
        ScopedDeclarationCompileTestCase(
            "private declaration",
            {},
            "sql/models/domain/orders.sql",
            'MODEL (description "Test model orders.", constants (_value 5));\nSELECT @const("_value") AS value\n',
            "SELECT 5 AS value",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scoped_declaration_when_compiling_model_then_only_lexically_visible_values_expand(
    test_case: ScopedDeclarationCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {"sqlbuild_project.toml": _PROJECT_FILE}
        | test_case.files
        | {test_case.model_path: test_case.model_sql},
    )

    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)

    assert inputs.model_inputs[0].query_sql == test_case.expected_sql
    assert inputs.scope_index.declarations


@pytest.mark.parametrize(
    "test_case",
    (
        ScopedDeclarationErrorTestCase(
            "sibling declaration",
            {
                "sql/models/one/_constants/value.sql": "CONSTANT (name value, value 1);",
                "sql/models/two/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
            },
            ("known but inaccessible", "sql/models/one/_constants/value.sql", "sql/models/two/orders.sql"),
        ),
        ScopedDeclarationErrorTestCase(
            "descendant declaration",
            {
                "sql/models/domain/child/_constants/value.sql": "CONSTANT (name value, value 1);",
                "sql/models/domain/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
            },
            ("known but inaccessible", "sql/models/domain/child/_constants/value.sql"),
        ),
        ScopedDeclarationErrorTestCase(
            "unknown declaration",
            {
                "sql/models/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("missing") AS value'
            },
            ("Unknown constant 'missing'", "Visible constants: none"),
        ),
        ScopedDeclarationErrorTestCase(
            "model private does not export to test",
            {
                "sql/models/orders.sql": "MODEL (description 'Test model orders.', constants (_value 1));\nSELECT 1 AS value",
                "tests/unit/orders_test.sql": (
                    'TEST ();\nWITH __expected__orders AS (SELECT @const("_value") AS value) '
                    "SELECT 1"
                ),
            },
            ("known but inaccessible", "sql/models/orders.sql", "tests/unit/orders_test.sql"),
        ),
        ScopedDeclarationErrorTestCase(
            "filename resemblance without expected relationship",
            {
                "sql/models/domain/_constants/value.sql": "CONSTANT (name model_value, value 9);",
                "sql/models/domain/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
                "tests/unit/orders_test.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__assert__valid AS (SELECT @const('model_value') WHERE FALSE) SELECT 1"
                ),
            },
            ("known but inaccessible", "sql/models/domain/_constants/value.sql"),
        ),
        ScopedDeclarationErrorTestCase(
            "unrelated production scope is not granted",
            {
                "sql/models/orders/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
                "sql/models/customers/_constants/value.sql": (
                    "CONSTANT (name customer_value, value 10);"
                ),
                "sql/models/customers/customers.sql": "MODEL (description 'Test model customers.');\nSELECT 1 AS value",
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS (SELECT @const('customer_value') AS value) SELECT 1"
                ),
            },
            ("known but inaccessible", "sql/models/customers/_constants/value.sql"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_inaccessible_or_unknown_declaration_when_compiling_then_diagnostic_is_distinct(
    test_case: ScopedDeclarationErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    with pytest.raises(CompileInputError) as error:
        compile_project_inputs(project_dir=tmp_path)

    for fragment in test_case.expected_error_fragments:
        assert fragment in str(error.value)


@pytest.mark.parametrize(
    "test_case",
    (
        ScopedDeclarationSurfaceTestCase(
            "inline model hook",
            {
                "sql/models/domain/_constants/value.sql": "CONSTANT (name value, value 11);",
                "sql/models/domain/orders.sql": (
                    "MODEL (description 'Test model orders.', pre_hooks [inline_sql(\"SELECT @const('value')\")]);\nSELECT 1"
                ),
            },
            lambda inputs: (
                cast(list[SqlHookEntry], inputs.model_inputs[0].config.values["pre_hooks"])[
                    0
                ].statement
            ),
            "SELECT 11",
        ),
        ScopedDeclarationSurfaceTestCase(
            "named hook definition path",
            {
                "sql/models/domain/_sqlbuild/_constants/value.sql": "CONSTANT (name value, value 12);",
                "sql/models/domain/_sqlbuild/_hooks/sql/typed.sql": 'HOOK (description "Test hook typed.");\nSELECT @const("value")',
                "sql/models/domain/orders.sql": 'MODEL (description "Test model orders.", post_hooks [sql("typed")]);\nSELECT 1',
            },
            lambda inputs: (
                cast(list[SqlHookEntry], inputs.model_inputs[0].config.values["post_hooks"])[
                    0
                ].statement
            ),
            "SELECT 12",
        ),
        ScopedDeclarationSurfaceTestCase(
            "function definition path",
            {
                "sql/functions/sql/domain/_constants/value.sql": "CONSTANT (name value, value 13);",
                "sql/functions/sql/domain/value.sql": (
                    'FUNCTION (description "Test function value.", returns INTEGER);\n@const("value")'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1",
            },
            lambda inputs: inputs.sql_function_inputs[0].body_sql,
            "13",
        ),
        ScopedDeclarationSurfaceTestCase(
            "standalone audit definition path",
            {
                "sql/models/domain/_sqlbuild/_constants/value.sql": "CONSTANT (name value, value 15);",
                "sql/models/domain/_sqlbuild/audits/singular/check.sql": (
                    'AUDIT ();\nSELECT @const("value") FROM __ref("orders") '
                    'JOIN __ref("customers") ON TRUE'
                ),
                "sql/models/domain/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1",
                "sql/models/domain/customers.sql": "MODEL (description 'Test model customers.');\nSELECT 1",
            },
            lambda inputs: inputs.audit_inputs[0].sql_body,
            'SELECT 15 FROM __ref("orders") JOIN __ref("customers") ON TRUE',
        ),
        ScopedDeclarationSurfaceTestCase(
            "generic audit definition path",
            {
                "sql/models/domain/_sqlbuild/_constants/value.sql": (
                    "CONSTANT (name value, value 18);"
                ),
                "sql/models/domain/_sqlbuild/_audits/generic/scoped.sql": (
                    'AUDIT ();\nSELECT * FROM __ref("@model") WHERE @const("value") = 18'
                ),
                "sql/models/domain/orders.sql": "MODEL (description 'Test model orders.', audits [scoped]);\nSELECT 1",
            },
            lambda inputs: inputs.audit_inputs[0].sql_body,
            'SELECT * FROM __ref("orders") WHERE 18 = 18',
        ),
        ScopedDeclarationSurfaceTestCase(
            "unit test authored path",
            {
                "tests/unit/domain/_constants/value.sql": "CONSTANT (name value, value 16);",
                "tests/unit/domain/orders_test.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    '__expected__orders AS (SELECT @const("value") AS value) SELECT 1'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
            },
            lambda inputs: inputs.test_inputs[0].sql_body,
            "WITH __ref__orders AS (SELECT 1 AS value), "
            "__expected__orders AS (SELECT 16 AS value) SELECT 1",
        ),
        ScopedDeclarationSurfaceTestCase(
            "scenario authored path",
            {
                "tests/scenarios/domain/_constants/value.sql": ("CONSTANT (name value, value 17);"),
                "tests/scenarios/domain/orders_scenario.sql": (
                    "SCENARIO (description 'Test scenario orders.');\nWITH __ref__orders AS (SELECT 1 AS value), "
                    '__expected__orders AS (SELECT @const("value") AS value) SELECT 1'
                ),
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
            },
            lambda inputs: inputs.scenario_inputs[0].sql_body,
            "WITH __ref__orders AS (SELECT 1 AS value), "
            "__expected__orders AS (SELECT 17 AS value) SELECT 1",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scoped_declaration_when_compiling_sql_surface_then_uses_authored_definition_path(
    test_case: ScopedDeclarationSurfaceTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)
    actual: str | None = test_case.result(inputs)

    assert actual == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    (
        ExpectedModelDeclarationGrantTestCase(
            description="single model inherited local constant and enum",
            files={
                "sql/models/domain/_constants/inherited.sql": (
                    "CONSTANT (name inherited_value, value 2);"
                ),
                "sql/models/domain/_constants/local.sql": ("CONSTANT (name local_value, value 3);"),
                "sql/models/domain/_enums/state.sql": "ENUM (name state, members [OPEN, CLOSED]);",
                "sql/models/domain/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value, 'OPEN' AS state",
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS (SELECT @const('inherited_value') + "
                    "@const('local_value') AS value, @enum('state').OPEN AS state) SELECT 1"
                ),
            },
            expected_sql_fragments=("SELECT 2 + 3 AS value", "'OPEN' AS state"),
            expected_grants=(
                ("constant:inherited_value", "model:orders", "test:check"),
                ("constant:local_value", "model:orders", "test:check"),
                ("enum:state", "model:orders", "test:check"),
            ),
        ),
        ExpectedModelDeclarationGrantTestCase(
            description="multiple expected models deterministic union",
            files={
                "sql/models/a/_constants/a.sql": "CONSTANT (name a_value, value 4);",
                "sql/models/a/a.sql": "MODEL (description 'Test model a.');\nSELECT 4 AS value",
                "sql/models/b/_constants/b.sql": "CONSTANT (name b_value, value 5);",
                "sql/models/b/b.sql": "MODEL (description 'Test model b.');\nSELECT 5 AS value",
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__a AS (SELECT 1 AS value), "
                    "__expected__b AS (SELECT @const('b_value') AS value), "
                    "__expected__a AS (SELECT @const('a_value') AS value) SELECT 1"
                ),
            },
            expected_sql_fragments=("SELECT 5 AS value", "SELECT 4 AS value"),
            expected_grants=(
                ("constant:a_value", "model:a", "test:check"),
                ("constant:b_value", "model:b", "test:check"),
            ),
        ),
        ExpectedModelDeclarationGrantTestCase(
            description="test path visibility remains separate from model grant",
            files={
                "sql/models/domain/_constants/model.sql": "CONSTANT (name model_value, value 6);",
                "sql/models/domain/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
                "tests/unit/_constants/test.sql": "CONSTANT (name test_value, value 7);",
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS (SELECT @const('model_value') + "
                    "@const('test_value') AS value) SELECT 1"
                ),
            },
            expected_sql_fragments=("SELECT 6 + 7 AS value",),
            expected_grants=(("constant:model_value", "model:orders", "test:check"),),
        ),
        ExpectedModelDeclarationGrantTestCase(
            description="only macros called by the test are granted",
            files={
                "sql/models/domain/_sqlbuild/_macros/python/discount.py": (
                    'def discount(expression: str) -> str:\n    return f"({expression} - 1)"\n'
                ),
                "sql/models/domain/_sqlbuild/_macros/python/surcharge.py": (
                    'def surcharge(expression: str) -> str:\n    return f"({expression} + 1)"\n'
                ),
                "sql/models/domain/orders.sql": (
                    "MODEL (description 'Test model orders.');\nSELECT @py.discount('10') AS value, @py.surcharge('10') AS fee"
                ),
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS (SELECT @py.discount('10') AS value) SELECT 1"
                ),
            },
            expected_sql_fragments=("SELECT (10 - 1) AS value",),
            expected_grants=(("macro:discount", "model:orders", "test:check"),),
        ),
        ExpectedModelDeclarationGrantTestCase(
            description="macros called inside macro arguments are granted",
            files={
                "sql/models/domain/_sqlbuild/_macros/python/discount.py": (
                    'def discount(expression: str) -> str:\n    return f"({expression} - 1)"\n'
                ),
                "sql/models/domain/_sqlbuild/_macros/python/surcharge.py": (
                    'def surcharge(expression: str) -> str:\n    return f"({expression} + 1)"\n'
                ),
                "sql/models/domain/orders.sql": (
                    "MODEL (description 'Test model orders.');\nSELECT @py.discount('10') AS value, @py.surcharge('10') AS fee"
                ),
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS (SELECT @py.discount(@py.surcharge('10')) AS value) SELECT 1"
                ),
            },
            expected_sql_fragments=("SELECT ((10 + 1) - 1) AS value",),
            expected_grants=(
                ("macro:discount", "model:orders", "test:check"),
                ("macro:surcharge", "model:orders", "test:check"),
            ),
        ),
        ExpectedModelDeclarationGrantTestCase(
            description="model private declaration is not granted",
            files={
                "sql/models/orders.sql": (
                    "MODEL (description 'Test model orders.', constants (_private 8));\nSELECT @const('_private') AS value"
                ),
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS (SELECT 1 AS value) SELECT 1"
                ),
            },
            expected_sql_fragments=("SELECT 1 AS value",),
            expected_grants=(),
        ),
        ExpectedModelDeclarationGrantTestCase(
            description="filename resemblance and no expected model grant nothing",
            files={
                "sql/models/domain/customers.sql": (
                    "MODEL (description 'Test model customers.');\nSELECT 1 AS value"
                ),
                "sql/models/domain/orders.sql": (
                    "MODEL (description 'Test model orders.');\n"
                    'SELECT value FROM __ref("customers")'
                ),
                "tests/unit/orders_test.sql": (
                    "TEST ();\nWITH __ref__customers AS (SELECT 1 AS value), __assert__valid AS "
                    '(SELECT value FROM __ref("orders") WHERE FALSE) SELECT 1'
                ),
            },
            expected_sql_fragments=("__assert__valid",),
            expected_grants=(),
        ),
        ExpectedModelDeclarationGrantTestCase(
            description="scenario expected model grant",
            files={
                "sql/models/domain/_constants/value.sql": "CONSTANT (name scenario_value, value 10);",
                "sql/models/domain/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
                "tests/scenarios/check.sql": (
                    "SCENARIO (description 'Test scenario check.');\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS (SELECT @const('scenario_value') AS value) SELECT 1"
                ),
            },
            expected_sql_fragments=("SELECT 10 AS value",),
            expected_grants=(("constant:scenario_value", "model:orders", "scenario:check"),),
            input_collection="scenario_inputs",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_expected_models_when_compiling_then_public_declarations_are_granted_with_provenance(
    test_case: ExpectedModelDeclarationGrantTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    first: CompileInputsView = compile_project_inputs(project_dir=tmp_path)
    second: CompileInputsView = compile_project_inputs(project_dir=tmp_path)
    compiled: CompiledProjectView = assemble_project(inputs=first, skip_column_inference=True)
    sql_body: str = getattr(first, test_case.input_collection)[0].sql_body
    grants: tuple[tuple[str, str, str], ...] = tuple(
        sorted(
            (
                f"{grant.declaration.kind.value}:{grant.declaration.name}",
                f"{grant.through.kind.value}:{grant.through.name}",
                f"{grant.resource.kind.value}:{grant.resource.name}",
            )
            for grant in first.scope_index.grants
        )
    )

    assert all(fragment in sql_body for fragment in test_case.expected_sql_fragments)
    assert grants == test_case.expected_grants
    assert first.scope_index.completeness.relationships
    assert first.scope_index == second.scope_index
    assert scope_metadata_projection(index=first.scope_index) == scope_metadata_projection(
        index=second.scope_index
    )
    assert compiled.scope_index.grants == first.scope_index.grants


@pytest.mark.parametrize(
    "test_case",
    (
        RelationshipUsageTestCase(
            description="expected model provenance is retained",
            files={
                "sql/models/domain/_constants/value.sql": ("CONSTANT (name model_value, value 6);"),
                "sql/models/domain/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
                "tests/unit/check.sql": (
                    "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
                    "__expected__orders AS "
                    "(SELECT @const('model_value') AS value) SELECT 1"
                ),
            },
            expected_usage=UsageRecord(
                consumer=ResourceIdentity(ResourceKind.TEST, "check"),
                declaration=DeclarationIdentity(DeclarationKind.CONSTANT, "model_value"),
                kind=UsageKind.RUNTIME,
                through=ResourceIdentity(ResourceKind.MODEL, "orders"),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_relationship_granted_reference_when_compiling_then_usage_retains_model_provenance(
    test_case: RelationshipUsageTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files,
    )

    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)
    compiled: CompiledProjectView = assemble_project(inputs=inputs, skip_column_inference=True)

    assert test_case.expected_usage in compiled.scope_index.usages


@pytest.mark.parametrize(
    "test_case",
    (
        ScopePlacementCompileTestCase(
            description="exact local placement is accepted",
            files={
                "sql/models/domain/_constants/value.sql": ("CONSTANT (name value, value 1);"),
                "sql/models/domain/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
            },
        ),
        ScopePlacementCompileTestCase(
            description="inherited lowest common ancestor is accepted",
            files={
                "sql/models/domain/constants/value.sql": "CONSTANT (name value, value 1);",
                "sql/models/domain/a/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
                "sql/models/domain/b/customers.sql": 'MODEL (description "Test model customers.");\nSELECT @const("value") AS value',
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_exact_declaration_placement_when_assembling_then_project_is_accepted(
    test_case: ScopePlacementCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files,
    )

    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)
    compiled: CompiledProjectView = assemble_project(inputs=inputs, skip_column_inference=True)

    assert compiled.scope_index.completeness.runtime_usage is test_case.expected_complete
    assert compiled.scope_index.completeness.placement is test_case.expected_complete


@pytest.mark.parametrize(
    "test_case",
    (
        ScopePlacementCompileTestCase(
            description="two models reuse directory scope",
            files={},
            expected_model_names=("customers", "orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_models_in_same_directory_when_reusing_visibility_then_usage_consumers_remain_exact(
    test_case: ScopePlacementCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/models/domain/area/_constants/value.sql": "CONSTANT (name value, value 1);",
            "sql/models/domain/area/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
            "sql/models/domain/area/customers.sql": 'MODEL (description "Test model customers.");\nSELECT @const("value") AS value',
        },
    )

    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)
    compiled: CompiledProjectView = assemble_project(inputs=inputs, skip_column_inference=True)

    consumers: set[ResourceIdentity] = {
        cast(ResourceIdentity, usage.consumer) for usage in compiled.scope_index.usages
    }
    assert {consumer.name for consumer in consumers} == set(test_case.expected_model_names)


@pytest.mark.parametrize(
    "test_case",
    (
        ScopePlacementCompileTestCase(
            description="used global with one owner is rejected as over broad",
            files={
                "sql/constants/value.sql": "CONSTANT (name value, value 1);",
                "sql/models/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
            },
            expected_fragment="required exact-owner-private at 'models'",
        ),
        ScopePlacementCompileTestCase(
            description="unused global is rejected",
            files={
                "sql/constants/value.sql": "CONSTANT (name value, value 1);",
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
            },
            expected_fragment="Unused global declaration 'constant:value'",
        ),
        ScopePlacementCompileTestCase(
            description="unused private is rejected",
            files={
                "sql/models/orders.sql": (
                    "MODEL (description 'Test model orders.', constants (_value 1));\nSELECT 1 AS value"
                ),
            },
            expected_fragment="Unused private declaration 'constant:model:orders._value'",
        ),
        ScopePlacementCompileTestCase(
            description="inherited declaration used in one directory must be local",
            files={
                "sql/models/domain/constants/value.sql": "CONSTANT (name value, value 1);",
                "sql/models/domain/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
            },
            expected_fragment="required exact-owner-private at 'sql/models/domain'",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_declaration_placement_when_assembling_then_it_reports_an_error(
    test_case: ScopePlacementCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files,
    )
    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)

    project: CompiledProjectView = assemble_project(inputs=inputs, skip_column_inference=True)

    assert re.search(
        cast(str, test_case.expected_fragment),
        render_compile_diagnostics(project=project),
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ScopePlacementCompileTestCase(
            description="over broad global is advisory when placement enforcement is disabled",
            files={
                "sql/constants/value.sql": "CONSTANT (name value, value 1);",
                "sql/models/orders.sql": 'MODEL (description "Test model orders.");\nSELECT @const("value") AS value',
            },
            expected_model_names=("orders",),
            expected_diagnostics=(("S024", "warning"),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_disabled_placement_enforcement_when_assembling_then_project_and_warning_are_produced(
    test_case: ScopePlacementCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    project_file: str = _PROJECT_FILE + "\n[scopes]\nenforce_placement = false\n"
    write_repo_files(
        tmp_path,
        {"sqlbuild_project.toml": project_file} | test_case.files,
    )
    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)

    compiled: CompiledProjectView = assemble_project(inputs=inputs, skip_column_inference=True)

    assert tuple(model.name for model in compiled.models) == test_case.expected_model_names
    assert (
        tuple(
            (diagnostic.code.value, diagnostic.severity.value)
            for diagnostic in compiled.scope_index.diagnostics
        )
        == test_case.expected_diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ScopedDeclarationCompileTestCase(
            description="inherited enum contract",
            files={"sql/models/domain/enums/state.sql": "ENUM (name state, members [OPEN, CLOSED]);"},
            model_path="sql/models/domain/child/orders.sql",
            model_sql=(
                "MODEL (description 'Test model orders.', contract enforced, columns (state (type state)));\n"
                'SELECT @enum("state").OPEN AS state'
            ),
            expected_sql="SELECT 'OPEN' AS state",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_inherited_enum_when_compiling_contract_then_resolves_from_model_path(
    test_case: ScopedDeclarationCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
        }
        | test_case.files
        | {test_case.model_path: test_case.model_sql},
    )

    model: ModelInput = compile_project_inputs(project_dir=tmp_path).model_inputs[0]

    assert model.query_sql == test_case.expected_sql
    assert model.schema_entry is not None
    assert model.schema_entry.columns[0].type == "VARCHAR"
    assert tuple(model.enum_columns) == ("state",)


@pytest.mark.parametrize(
    "test_case",
    (
        ScopedDeclarationCompileTestCase(
            description="source authored path",
            files={
                "sql/sources/domain/_constants/value.sql": "CONSTANT (name value, value 14);",
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1",
            },
            model_path="sql/sources/domain/source.yml",
            model_sql='SELECT @const("value")',
            expected_sql="SELECT 14",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scoped_declaration_when_compiling_source_then_uses_authored_source_path(
    test_case: ScopedDeclarationCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)
    source_path: Path = tmp_path / test_case.model_path
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    discovered = replace(
        discovered,
        source_files=(
            DiscoveredSourceFile(
                file_path=source_path,
                relative_path=Path(test_case.model_path),
                contents="",
                source_entries=(SourceEntry(name="raw", expression=test_case.model_sql),),
            ),
        ),
    )

    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="source_scope_test",
    )

    assert inputs.source_inputs[0].source_entry.expression == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    (
        ScopePlacementCompileTestCase(
            description="two sources reuse file scope",
            files={},
            expected_model_names=("customer_orders", "inventory_orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sources_in_same_file_when_reusing_visibility_then_usage_consumers_remain_exact(
    test_case: ScopePlacementCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    source_path: Path = tmp_path / "sql/sources/domain/source.yml"
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/sources/domain/_constants/value.sql": "CONSTANT (name value, value 14);",
            "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1",
        },
    )
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    discovered = replace(
        discovered,
        source_files=(
            DiscoveredSourceFile(
                file_path=source_path,
                relative_path=Path("sql/sources/domain/source.yml"),
                contents="",
                source_entries=(
                    SourceEntry(name="customer_orders", expression='SELECT @const("value")'),
                    SourceEntry(name="inventory_orders", expression='SELECT @const("value")'),
                ),
            ),
        ),
    )

    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="source_scope_reuse_test",
    )

    source_usages: Iterator[UsageRecord] = chain.from_iterable(
        source.declaration_usages for source in inputs.source_inputs
    )
    consumers: set[ResourceIdentity | DeclarationIdentity] = {
        usage.consumer for usage in source_usages
    }
    assert {consumer.name for consumer in consumers} == set(test_case.expected_model_names)


@pytest.mark.parametrize(
    "test_case",
    (
        ScopePlacementCompileTestCase(
            description="two tests reuse directory scope",
            files={},
            expected_model_names=("customer_orders", "inventory_orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_tests_in_same_directory_when_reusing_visibility_then_usage_consumers_remain_exact(
    test_case: ScopePlacementCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    test_sql: str = (
        "TEST ();\nWITH __ref__orders AS (SELECT 1 AS value), "
        '__expected__orders AS (SELECT @const("value") AS value) SELECT 1'
    )
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS value",
            "tests/unit/domain/_constants/value.sql": "CONSTANT (name value, value 16);",
            "tests/unit/domain/customer_orders.sql": test_sql,
            "tests/unit/domain/inventory_orders.sql": test_sql,
        },
    )

    inputs: CompileInputsView = compile_project_inputs(project_dir=tmp_path)

    test_usages: Iterator[UsageRecord] = chain.from_iterable(
        test.declaration_usages for test in inputs.test_inputs
    )
    consumers: set[ResourceIdentity | DeclarationIdentity] = {
        usage.consumer for usage in test_usages
    }
    assert {consumer.name for consumer in consumers} == set(test_case.expected_model_names)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
