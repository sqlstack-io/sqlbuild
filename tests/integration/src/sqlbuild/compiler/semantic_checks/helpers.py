from __future__ import annotations

import functools
import random
import re
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass, replace
from itertools import chain
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import sqlbuild._native as native_module
from sqlbuild._native import (
    CompilerDiagnostic,
    NativeCompilerError,
    project_with,
)
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledModelView, CompiledProjectView
from tests.integration.src.sqlbuild.compiler.analysis_session.helpers import (
    compile_inputs,
    generated_analysis_files,
)

if TYPE_CHECKING:
    from sqlbuild._native import DiagnosticRow

_NATIVE_SESSION_REQUIRED: str = "revalidating a native route without its session"
_RELATIONS: dict[str, tuple[str, ...]] = {
    '__source("raw_orders")': (
        "order_id",
        "customer_id",
        "amount",
        "status",
        "ordered_at",
        "order_date",
    ),
}
_SOURCES: str = """sources:
  - name: raw_orders
    description: Orders feed.
    expression: >-
      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount, 'placed' AS status,
      TIMESTAMP '2026-01-01 00:00:00' AS ordered_at, DATE '2026-01-01' AS order_date)
    columns:
      - name: order_id
        type: INTEGER
      - name: customer_id
        type: INTEGER
      - name: amount
        type: DOUBLE
      - name: status
        type: VARCHAR
      - name: ordered_at
        type: TIMESTAMP
      - name: order_date
        type: DATE
"""
_OPT_OUT_SHARE: float = 0.2
_NON_ASCII_NAMES: list[tuple[str, str]] = [
    ("amount", "am\u00f6unt"),
    ("status", "\u017ftatus"),
    ("customer_id", "customer_\u0130d"),
    ("order_date", "order_d\u0101te"),
    ("value", "va\u212alue"),
    ("ordered_at", "ordered_\u0430t"),
]
_NON_ASCII_COMMENTS: tuple[str, ...] = (
    " /* caf\u00e9 \u00df \u2028 \u0131 */ ",
    " -- na\u00efve\u0085x\n",
    " /* \u0663\u0664 \u3000 */ ",
    " ",
)
_TYPO_SHARE: float = 0.45
_MIN_TYPO_LENGTH: int = 4
_ALIASES: tuple[tuple[str, str], ...] = (("", ""), ("o.", " AS o"))
_SETTINGS: tuple[str, str] = ("", "\n[settings]\nrequire_sql_analysis = true\n")
_OPT_OUTS: tuple[str, str] = ("", ", sql_analysis false")
_SOURCE_COMPARISON_SHARE: float = 0.6

type _Template = Callable[[random.Random, str, tuple[str, ...]], tuple[str, tuple[str, ...]]]


@dataclass(frozen=True)
class SemanticInputs:
    """The arguments the compile passed to `complete_semantic_project`."""

    project: CompiledProjectView
    profile: ExpressionInferenceProfile
    binding_results: dict[str, list[DiagnosticRow]]
    resource_sql_analysis: bool
    session: Any | None = None


def _typo(rng: random.Random, column: str) -> str:
    index: int = rng.randrange(1, max(len(column) - 2, 2))
    swapped: str = (
        column[:index] + column[index + 1 : index + 2] + column[index] + column[index + 2 :]
    )
    return (column, swapped)[len(column) >= _MIN_TYPO_LENGTH and rng.random() < _TYPO_SHARE]


def _projection(
    rng: random.Random, relation: str, columns: tuple[str, ...]
) -> tuple[str, tuple[str, ...]]:
    picked: list[str] = rng.sample(list(columns), k=min(len(columns), rng.randint(2, 4)))
    written: list[str] = [_typo(rng, column) for column in picked]
    prefix, suffix = rng.choice(_ALIASES)
    sql: str = f"SELECT {', '.join(prefix + name for name in written)}\nFROM {relation}{suffix}"
    intended: tuple[str, ...] = tuple(
        rng.choice((name, meant)) for name, meant in zip(written, picked, strict=True)
    )
    return sql, intended


def _comparison(
    rng: random.Random, relation: str, columns: tuple[str, ...]
) -> tuple[str, tuple[str, ...]]:
    relation, columns = ((relation, columns), next(iter(_RELATIONS.items())))[
        rng.random() < _SOURCE_COMPARISON_SHARE
    ]
    column: str = rng.choice(columns)
    literal: str = rng.choice(("5", "'placed'", "DATE '2026-01-01'", "TIMESTAMP '2026-01-01'"))
    kept: tuple[str, ...] = tuple(dict.fromkeys((columns[0], column)))
    sql: str = (
        f"SELECT {', '.join('o.' + name for name in kept)}\nFROM {relation} AS o\n"
        f"WHERE o.{_typo(rng, column)} > {literal}"
    )
    return sql, kept


def _arithmetic(
    rng: random.Random, relation: str, columns: tuple[str, ...]
) -> tuple[str, tuple[str, ...]]:
    left, right = rng.sample(list(columns), k=2)
    sql: str = f"SELECT {columns[0]}, {left} + {right} AS {left}\nFROM {relation}"
    return sql, tuple(dict.fromkeys((columns[0], left)))


def _aggregate(
    rng: random.Random, relation: str, columns: tuple[str, ...]
) -> tuple[str, tuple[str, ...]]:
    key, value = rng.sample(list(columns), k=2)
    sql: str = (
        f"WITH base AS (\n  SELECT {key}, {_typo(rng, value)} FROM {relation}\n)\n"
        f"SELECT {key}, MAX({value}) AS {value}\nFROM base\nGROUP BY 1"
    )
    return sql, (key, value)


_TEMPLATES: tuple[_Template, ...] = (_projection, _comparison, _arithmetic, _aggregate)


def _has_two_columns(relation: tuple[str, tuple[str, ...]]) -> bool:
    return len(relation[1]) > 1


def generated_semantic_files(
    *, rng: random.Random, model_count: int, require_analysis: bool
) -> dict[str, str]:
    """A project whose models chain through typos, temporal comparisons and type errors."""

    settings: str = _SETTINGS[require_analysis]
    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "orders_semantics"\nadapter = "duckdb"\n\n[connection]\n'
            f'database = "orders.duckdb"\n{settings}'
        ),
        "sources/raw.yml": _SOURCES,
    }
    relations: list[tuple[str, tuple[str, ...]]] = list(_RELATIONS.items())
    for index in range(model_count):
        relation, columns = rng.choice(list(filter(_has_two_columns, relations)))
        sql, outputs = _TEMPLATES[index % len(_TEMPLATES)](rng, relation, columns)
        name: str = f"orders_model_{index}"
        opt_out: str = _OPT_OUTS[rng.random() < _OPT_OUT_SHARE]
        files[f"models/layer_{index % 3}/{name}.sql"] = (
            f'MODEL (description "Generated orders model {index}"{opt_out});\n\n{sql}\n'
        )
        relations.append((f'__ref("{name}")', outputs))
    return files


def captured_semantic_inputs(
    *,
    project_dir: Path,
    files: dict[str, str],
) -> SemanticInputs:
    """Compile and keep the semantic completion stage's inputs."""

    stage: dict[str, Any] | None = native_module.assembly_stage_inputs(
        "semantic_completion",
        inputs=compile_inputs(project_dir=project_dir, files=files),
        inference_profile=DuckDbAdapter().expression_inference_profile(),
    )
    assert stage is not None
    return SemanticInputs(**stage)


def captured_under_dialect(
    *,
    project_dir: Path,
    files: dict[str, str],
    dialect: str,
) -> SemanticInputs:
    """Compile under `dialect`, with DuckDB's function rules, and keep the stage's inputs."""

    stage: dict[str, Any] | None = native_module.assembly_stage_inputs(
        "semantic_completion",
        inputs=compile_inputs(project_dir=project_dir, files=files),
        inference_profile=replace(
            DuckDbAdapter().expression_inference_profile(), sql_analysis_dialect=dialect
        ),
    )
    assert stage is not None
    return SemanticInputs(**stage)


def with_dialect(inputs: SemanticInputs, dialect: str | None) -> SemanticInputs:
    """The same stage inputs read under another SQL dialect."""

    return replace(
        inputs,
        project=project_with(project=inputs.project, sql_analysis_dialect=dialect),
        profile=replace(inputs.profile, sql_analysis_dialect=dialect),
    )


def completed_natively(inputs: SemanticInputs) -> CompiledProjectView:
    """The native stage's completed project."""

    return native_completion(inputs)


def note_kinds(project: CompiledProjectView, kinds: tuple[str, ...]) -> Counter[str]:
    """How many notes of each kind the project's diagnostics carry."""

    notes: list[str] = list(chain.from_iterable(item.notes for item in project.diagnostics))
    counts: Counter[str] = Counter()
    for kind in kinds:
        counts[kind] = sum(kind in note for note in notes)
    return counts


def native_completion(inputs: SemanticInputs) -> CompiledProjectView:
    """The native stage's completed project."""

    return native_module.complete_semantic_project(
        project=inputs.project,
        profile=inputs.profile,
        binding_results=inputs.binding_results,
        resource_sql_analysis=inputs.resource_sql_analysis,
        session=inputs.session,
    )


def session_corpus_files(*, corpus: str, rng: random.Random, model_count: int) -> dict[str, str]:
    """One generated project from the failing-semantics or the analysis-session corpus."""

    generators: dict[str, Callable[[], dict[str, str]]] = {
        "semantic": lambda: generated_semantic_files(
            rng=rng, model_count=model_count, require_analysis=False
        ),
        "analysis": lambda: generated_analysis_files(rng=rng, model_count=model_count),
    }
    return generators[corpus]()


def proven_output_count(project: CompiledProjectView) -> int:
    """How many models a dynamic pivot proof gave their output names."""

    return sum(
        model.dynamic_column_contract is not None and model.dynamic_column_contract.output_proven
        for model in project.models
    )


def without_session(inputs: SemanticInputs) -> SemanticInputs:
    """The same stage inputs with every model's output names and lineage in the payload."""

    return replace(inputs, session=None)


def payload_completion_view(inputs: SemanticInputs) -> tuple[object, ...] | None:
    """The completion from payload facts alone; `None` where a native route requires the session."""

    try:
        completed: CompiledProjectView = completed_natively(without_session(inputs))
    except NativeCompilerError as error:
        assert _NATIVE_SESSION_REQUIRED in str(error)
        return None
    return completion_view(completed)


def session_fact_models(inputs: SemanticInputs) -> frozenset[str]:
    """Models whose output names and lineage the finished session supplies to the stage."""

    kept: frozenset[str] = frozenset(getattr(inputs.session, "fact_models", ()))
    proven: frozenset[str] = frozenset(
        model.name for model in filter(_output_proven, inputs.project.models)
    )
    return frozenset(model.name for model in inputs.project.models) & kept - proven


def _output_proven(model: CompiledModelView) -> bool:
    return model.dynamic_column_contract is not None and model.dynamic_column_contract.output_proven


def has_blocking_binding(project: CompiledProjectView) -> bool:
    """Whether any model owns a binding error, so the stage plans a type recovery."""

    bindings: list[tuple[CompilerDiagnostic, ...]] = [
        model.binding_diagnostics for model in project.models
    ]
    return any(item.is_error for item in chain.from_iterable(bindings))


def completion_view(project: CompiledProjectView) -> tuple[object, ...]:
    """Everything the stage decides: diagnostics and each model's binding and type facts."""

    return (
        project.diagnostics,
        tuple(_model_view(model) for model in project.models),
    )


def _model_view(model: CompiledModelView) -> tuple[object, ...]:
    return (
        model.name,
        model.binding_diagnostics,
        model.inferred_columns,
        model.unchecked_output_columns,
    )


def non_ascii_variant(*, files: dict[str, str], rng: random.Random) -> dict[str, str]:
    """The project with some column names spelt beyond ASCII and a comment after each model
    header holding non-ASCII text, digits and Python-only line separators."""

    renamed: list[tuple[str, str]] = rng.sample(_NON_ASCII_NAMES, k=rng.randint(1, 6))
    return {
        path: _non_ascii_text(path=path, text=text, renamed=renamed, rng=rng)
        for path, text in files.items()
    }


def files_with_non_ascii(
    *, files: dict[str, str], rng: random.Random, non_ascii: bool
) -> dict[str, str]:
    """`files`, or with `non_ascii` their `non_ascii_variant`."""

    variants: tuple[Callable[[], dict[str, str]], ...] = (
        lambda: files,
        lambda: non_ascii_variant(files=files, rng=rng),
    )
    return variants[non_ascii]()


def _non_ascii_text(
    *, path: str, text: str, renamed: list[tuple[str, str]], rng: random.Random
) -> str:
    renamed_text: str = functools.reduce(
        lambda current, names: re.sub(rf"\b{names[0]}\b", names[1], current), renamed, text
    )
    commented: bool = path.endswith(".sql") and ";" in renamed_text
    comments: tuple[Callable[[], str], ...] = (
        lambda: "",
        lambda: rng.choice(_NON_ASCII_COMMENTS),
    )
    comment: str = comments[commented]()
    header_end: int = renamed_text.find(";") + 1
    return renamed_text[:header_end] + comment + renamed_text[header_end:]


def semantic_outcome(complete: Callable[[], object]) -> tuple[str, object]:
    """`("completed", result)`, or the type and message of the error the stage raised."""

    try:
        return ("completed", complete())
    except Exception as error:  # noqa: BLE001 - the error is the outcome under comparison
        return (type(error).__name__, str(error))


def outcome_label(outcome: tuple[str, object]) -> str:
    """`completed`, or the error as `Type: message`."""

    kind, value = outcome
    return {"completed": kind}.get(kind, f"{kind}: {value}")


_SOURCE_COLUMNS: tuple[str, ...] = (
    "order_id",
    "customer_id",
    "amount",
    "status",
    "ordered_at",
    "order_date",
)
_MODEL_COLUMNS: tuple[str, ...] = ("order_id", "customer_id", "ordered_at", "value")
_FUNCTIONS: dict[str, tuple[str, str, str, int]] = {
    "scaled_amount": ("p_amount DOUBLE, p_factor INTEGER", "DOUBLE", "p_amount * p_factor", 2),
    "label_status": ("p_status VARCHAR", "VARCHAR", "UPPER(p_status)", 1),
    "days_since": ("p_at TIMESTAMP", "INTEGER", "DATE_DIFF('day', p_at, NOW())", 1),
}
_ARGUMENTS: tuple[str, ...] = (
    "o.{column}",
    "{column}",
    "x.{column}",
    "'x'",
    "2",
    "2.5",
    "CAST({column} AS VARCHAR)",
    "CAST({column} AS DECIMAL(10, 2))",
    "TRUE",
    "{column} = 1",
)
_ARITY_CHANGES: tuple[int, ...] = (0, 0, 0, 1, -1)
_UNIQUE_KEYS: tuple[str, ...] = ("order_id", "customer_id", "order_key")
_CURSORS: tuple[str, ...] = ("ordered_at", "customer_id", "value", "placed_at")
_CURSOR_TYPES: tuple[str, ...] = ("timestamp", "integer")
_GRAINS: dict[str, str] = {"timestamp": "  cursor_grain day,\n"}
_INPUT_COLUMNS: tuple[str, ...] = ("ordered_at", "order_id", "created_at")
_LOADED_CURSORS: tuple[str, ...] = ("load_seq", "Load_Seq", "loaded_at")
_LOADED_SOURCE: str = (
    "  - name: raw_events\n    description: Loaded events.\n    managed: true\n"
    "    write_strategy: append\n    contract: enforced\n    cursor_column: {cursor}\n"
    "    columns:\n"
    "      - name: event_id\n        type: INTEGER\n"
    "      - name: load_seq\n        type: INTEGER\n"
)
_LOADER: str = (
    "from sqlbuild.loaders import loader\n\n\n@loader\n"
    "def raw_events(ctx):\n    '''Load events.'''\n    return [{'event_id': 1, 'load_seq': 1}]\n"
)
_FIXTURE_EXTRAS: tuple[str, ...] = ("", ", 2 AS discount")
_EXPECTED_EXTRAS: tuple[str, ...] = ("", ", 3 AS Customer_ID", ", 2 AS refund")
_CTE_SHARE: float = 0.3
_AUDITS: tuple[str, ...] = (
    "",
    "  columns (\n    customer_id (audits [accepted_values (values [DATE '2026-01-01'])]),\n  ),\n",
    '  columns (\n    order_id (audits [relationships (to __ref("orders_model_0"),'
    " field order_key)]),\n  ),\n",
)
_SOURCE_SQL: str = (
    "    expression: >-\n"
    "      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount, 'placed' AS"
    " status,\n      TIMESTAMP '2026-01-01 00:00:00' AS ordered_at, DATE '2026-01-01' AS"
    " order_date)\n    columns:\n"
)
_SOURCE_TYPES: tuple[str, ...] = ("INTEGER", "INTEGER", "DOUBLE", "VARCHAR", "TIMESTAMP", "DATE")

type _Relation = tuple[str, str, tuple[str, ...]]


def _function_file(name: str) -> str:
    arguments, returns, body, _ = _FUNCTIONS[name]
    return (
        f'FUNCTION (\n  description "Generated {name}",\n  arguments ({arguments}),\n'
        f"  returns {returns},\n);\n\n{body}\n"
    )


def _call(rng: random.Random, columns: tuple[str, ...], alias: str) -> str:
    name: str = rng.choice(tuple(_FUNCTIONS))
    count: int = max(0, _FUNCTIONS[name][3] + rng.choice(_ARITY_CHANGES))
    arguments: list[str] = [
        rng.choice(_ARGUMENTS).format(column=rng.choice(columns)).replace("o.", f"{alias}.")
        for _ in range(count)
    ]
    return f'__udf("{name}")({", ".join(arguments)})'


def _query(rng: random.Random, relation: _Relation) -> str:
    reference, _, columns = relation
    plain: str = (
        "SELECT o.order_id, o.customer_id, o.ordered_at, "
        f"{_call(rng, columns, 'o')} AS value\nFROM {reference} AS o"
    )
    with_cte: str = (
        f"WITH base AS (\n  SELECT * FROM {reference} AS o\n)\n"
        f"SELECT b.order_id, b.customer_id, b.ordered_at, {_call(rng, columns, 'b')} AS value\n"
        "FROM base AS b"
    )
    return (plain, with_cte)[rng.random() < _CTE_SHARE]


def _header(rng: random.Random, index: int, relation: _Relation) -> str:
    _, upstream, _ = relation
    cursor_type: str = rng.choice(_CURSOR_TYPES)
    incremental: str = (
        "  materialized incremental,\n  incremental_strategy delete_insert,\n"
        f"  unique_key [order_id],\n  cursor {rng.choice(_CURSORS)},\n"
        f"  cursor_type {cursor_type},\n{_GRAINS.get(cursor_type, '')}"
        f"  cursor_inputs (\n    {upstream} {rng.choice(_INPUT_COLUMNS)},\n  ),\n"
    )
    table: str = f"  materialized table,\n  unique_key [{rng.choice(_UNIQUE_KEYS)}],\n"
    configs: tuple[str, str, str] = (table, incremental, "")
    kind: int = (index % 3, 0)[index % 3 == 1 and not upstream]
    return configs[kind] + rng.choice(_AUDITS)


def generated_metadata_files(*, rng: random.Random, model_count: int) -> dict[str, str]:
    """A project calling declared functions, with config, source cursor and SQL test metadata."""

    columns: str = "".join(
        f"      - name: {name}\n        type: {kind}\n"
        for name, kind in zip(_SOURCE_COLUMNS, _SOURCE_TYPES, strict=True)
    )
    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "orders_metadata"\nadapter = "duckdb"\n\n[connection]\n'
            'database = "orders.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Orders feed.\n"
            f"{_SOURCE_SQL}{columns}{_LOADED_SOURCE.format(cursor=rng.choice(_LOADED_CURSORS))}"
        ),
        "python/loaders/raw_events.py": _LOADER,
        "tests/unit/test_orders_model_0.sql": (
            "TEST();\n\nWITH\n__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount,"
            " 'placed' AS status, TIMESTAMP '2026-01-01' AS ordered_at,"
            f" DATE '2026-01-01' AS order_date{rng.choice(_FIXTURE_EXTRAS)}\n),\n"
            "__expected__orders_model_0 AS (\n"
            f"  SELECT 1 AS order_id{rng.choice(_EXPECTED_EXTRAS)}\n)\nSELECT 1\n"
        ),
    }
    for name in _FUNCTIONS:
        files[f"functions/sql/{name}.sql"] = _function_file(name)
    relations: list[_Relation] = [('__source("raw_orders")', "", _SOURCE_COLUMNS)]
    for index in range(model_count):
        relation: _Relation = rng.choice(relations)
        name: str = f"orders_model_{index}"
        files[f"models/layer_{index % 3}/{name}.sql"] = (
            f'MODEL (\n  description "Generated orders model {index}",\n'
            f"{_header(rng, index, relation)});\n\n{_query(rng, relation)}\n"
        )
        relations.append((f'__ref("{name}")', name, _MODEL_COLUMNS))
    return files


def native_metadata(inputs: SemanticInputs) -> tuple[CompilerDiagnostic, ...]:
    """The native stage's metadata diagnostics."""

    return native_module.semantic_metadata_diagnostics(
        project=inputs.project,
        profile=inputs.profile,
        resource_sql_analysis=inputs.resource_sql_analysis,
    )


def _completed(outcome: tuple[str, object]) -> bool:
    return outcome[0] == "completed"


def metadata_families(views: dict[str, object]) -> Counter[str]:
    """Completed metadata runs and their diagnostics counted by resource type and code."""

    completed: list[tuple[CompilerDiagnostic, ...]] = [
        cast(tuple[CompilerDiagnostic, ...], value)
        for _, value in filter(_completed, cast(list[tuple[str, object]], list(views.values())))
    ]
    families: Counter[str] = Counter(
        f"{item.resource_type} {item.code}" for item in chain.from_iterable(completed)
    )
    families["native"] = len(completed)
    return families
