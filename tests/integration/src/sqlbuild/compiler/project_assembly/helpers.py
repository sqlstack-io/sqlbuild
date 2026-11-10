"""Generated projects and assembly views for native project assembly."""

from __future__ import annotations

import datetime
import os
import random
from collections import Counter
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING, Any

import pytest

import sqlbuild._native as _native
from sqlbuild._native import (
    CollectionRendering,
    CompileAdapterContext,
    assemble_project,
    build_compile_inputs,
    discover_project_inputs,
    inputs_with,
    record_compile_input_reads,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

_PYTHON_RESOURCE_FUNCTIONS: tuple[str, ...] = ("validate_sql_syntax", "validate_hook_sql_syntax")
_DEFAULTS: tuple[str, ...] = (
    "",
    'seed_schema = "raw"\n',
    'database = "db_${env_name}"\nschema = "main"\n',
    'seed_database = "${n}"\nseed_schema = "seeds_${env_name}"\n',
)
_TARGETS: tuple[str, ...] = (
    'schema = "dev_${env_name}"\nloader_schema = "loaders"\n',
    'database = "wh_${n}"\nschema = "main"\n',
    'schema = "dev"\n',
    'database = "wh"\nschema = "analytics"\nloader_schema = "raw_${flag}"\n',
    "schema = \"${coalesce(ENV:SQB_ASSEMBLY_UNSET, 'main')}\"\n"
    'loader_schema = "${ENV:SQB_ASSEMBLY_LOADERS}"\n',
)
_SEED_NAMESPACES: tuple[str, ...] = (
    "",
    "    schema: seeds_${env_name}\n",
    '    schema: "${CTX:model.name}_s"\n',
    '    database: "${n}"\n    schema: "${CTX:model.database}_x"\n',
    '    schema: "${flag}"\n',
    '    schema: "${ENV:SQB_ASSEMBLY_LOADERS}_${CTX:model.alias}"\n',
)
_SOURCE_NAMESPACES: tuple[str, ...] = ("", "    schema: landing\n", "    database: lake\n")
_HOOKS: tuple[str, ...] = (
    "",
    '  pre_hooks [inline_sql("SELECT 0 AS n")],\n',
    "  post_hooks [inline_sql('SELECT 1'), inline_sql('SELECT 2')],\n",
    "  pre_hooks [inline_sql('SELECT 3')],\n  post_hooks [inline_sql('SELECT 4')],\n",
)
_AUDITS: tuple[str, ...] = ("", "  columns (n (audits [not_null])),\n")


def generated_assembly_files(*, rng: random.Random, model_count: int) -> dict[str, str]:
    """Return a project with managed sources, templated seeds, hooks, audits and a function."""

    references: list[str] = [
        '__source("raw_orders")',
        '__source("landing_orders")',
        '__seed("countries")',
        '__seed("regions")',
        *[f'__ref("orders_{index}")' for index in range(model_count - 1)],
    ]
    models: dict[str, str] = {
        f"models/orders_{index}.sql": (
            f'MODEL (\n  description "Generated orders model {index}",\n'
            f"{rng.choice(_HOOKS)}{rng.choice(_AUDITS)});\n\n"
            f"SELECT 1 AS n FROM {rng.choice(references[: 4 + index])}\n"
        )
        for index in range(model_count)
    }
    return {
        "sqlbuild_project.toml": (
            'name = "assembly"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
            '[connection]\ndatabase = ":memory:"\n\n'
            '[vars]\nenv_name = "prod"\nn = "3"\nflag = "true"\n\n'
            f"[defaults]\n{rng.choice(_DEFAULTS)}\n[targets.dev]\n{rng.choice(_TARGETS)}"
        ),
        "seeds/lookups.yml": (
            "seeds:\n"
            f"  - name: countries\n    description: Countries.\n{rng.choice(_SEED_NAMESPACES)}"
            "    columns:\n      - name: n\n        type: INTEGER\n"
            f"  - name: regions\n    description: Regions.\n{rng.choice(_SEED_NAMESPACES)}"
            "    columns:\n      - name: n\n        type: INTEGER\n"
        ),
        "seeds/countries.csv": "n\n1\n",
        "seeds/regions.csv": "n\n2\n",
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Raw orders.\n    managed: true\n"
            f"    write_strategy: table\n{rng.choice(_SOURCE_NAMESPACES)}"
            "    columns:\n      - name: n\n        type: INTEGER\n"
            "  - name: landing_orders\n    description: Landing orders.\n"
            f"{rng.choice(_SOURCE_NAMESPACES)}"
            "    columns:\n      - name: n\n        type: INTEGER\n"
        ),
        "python/loaders/raw.py": (
            "from sqlbuild.loaders import loader\n\n\n"
            "@loader\ndef raw_orders(ctx):\n    '''Load raw orders.'''\n    return [{'n': 1}]\n"
        ),
        "functions/sql/order_numbers.sql": (
            'FUNCTION (\n  description "Order numbers.",\n  arguments (p_n INTEGER),\n'
            "  returns table (n INTEGER)\n);\n\n"
            'SELECT n FROM __ref("orders_0") WHERE n = p_n\n'
        ),
        **models,
    }


def windows_environ(environment: dict[str, str]) -> os._Environ[str]:
    """Return this process's environment plus `environment` with Windows' upper-cased keys."""

    data: dict[str, str] = {
        name.upper(): value for name, value in {**os.environ, **environment}.items()
    }
    return os._Environ(data, str.upper, str, str, str)


def seed_yml(schema: str) -> str:
    """Return a one-seed declaration whose schema is `schema`."""

    return (
        "seeds:\n  - name: countries\n    description: Countries.\n"
        f'    schema: "{schema}"\n    columns:\n      - name: n\n        type: INTEGER\n'
    )


def project_inputs(
    *,
    project_dir: Path,
    files: dict[str, str],
    sql_analysis_dialect: str | None = DuckDbAdapter.sql_analysis_dialect_name,
) -> CompileInputsView:
    """Write `files` and attach their compile inputs as the compile pipeline does."""

    _ = [
        _write(path=project_dir / relative_path, contents=contents)
        for relative_path, contents in files.items()
    ]
    return build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter_context=CompileAdapterContext(
            value_renderer=DuckDbAdapter(),
            collection_rendering=CollectionRendering.VALUE_LIST,
            python_functions_inherit_default_namespace=True,
            sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
            sql_analysis_dialect=sql_analysis_dialect,
        ),
        run_id="integration_run",
        defer_model_sql_validation=True,
    )


def assemble_with(
    *, inputs: CompileInputsView, monkeypatch: pytest.MonkeyPatch
) -> CompiledProjectView:
    """Assemble `inputs`."""

    return assemble_project(inputs=inputs)


def recorded_assembly(
    *, inputs: CompileInputsView, monkeypatch: pytest.MonkeyPatch
) -> tuple[object, tuple[tuple[str, ...], bool]]:
    """Assemble `inputs`; return its assembly view and the input reads recorded."""

    with record_compile_input_reads() as reads:
        project: CompiledProjectView = assemble_with(inputs=inputs, monkeypatch=monkeypatch)
    return assembly_view(project), (reads.environment_names, reads.read_run_id)


def assembly_outcome(
    *,
    project_dir: Path,
    files: dict[str, str],
    effective_vars: dict[str, object] | None,
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[object, ...]:
    """Attach and assemble `files`: `("ok", view)`, or the error's class, message, help and code."""

    try:
        inputs: CompileInputsView = project_inputs(project_dir=project_dir, files=files)
        merged: CompileInputsView = inputs_with(
            inputs=inputs, effective_vars={**inputs.effective_vars, **(effective_vars or {})}
        )
        return (
            "ok",
            assembly_view(assemble_with(inputs=merged, monkeypatch=monkeypatch)),
        )
    except Exception as error:
        return (
            type(error).__name__,
            str(error).replace(str(project_dir), "<dir>"),
            getattr(error, "help", None),
            getattr(error, "code", None),
        )


def assembly_view(project: CompiledProjectView) -> object:
    """Return every resource fact native project assembly derives, with the diagnostics."""

    return (
        tuple((model.name, model.deps, model.destination) for model in project.models),
        tuple(source.source_entry for source in project.sources),
        tuple(seed.destination for seed in project.seeds),
        tuple((function.name, function.deps) for function in project.functions),
        tuple((audit.name, audit.scope_deps) for audit in project.audits),
        project.diagnostics,
    )


def rejected_opt_outs(
    *,
    project_dir: Path,
    files: dict[str, str],
    monkeypatch: pytest.MonkeyPatch,
) -> dict[str, bool]:
    """Attach `files`; return whether each opt-out is rejected."""

    inputs: CompileInputsView = project_inputs(project_dir=project_dir, files=files)
    return {
        model_input.model_file.file_path.stem: model_input.rejected_sql_analysis_opt_out is not None
        for model_input in inputs.model_inputs
    }


def python_resource_calls(*, monkeypatch: pytest.MonkeyPatch) -> Counter[str]:
    """Count calls to Python's model syntax error reporting from project assembly."""

    calls: Counter[str] = Counter()
    _ = [
        monkeypatch.setattr(
            _native,
            name,
            _counted(name=name, function=getattr(_native, name), calls=calls),
        )
        for name in _PYTHON_RESOURCE_FUNCTIONS
    ]
    return calls


def _write(*, path: Path, contents: str) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    return path.write_text(contents, encoding="utf-8")


def _counted(*, name: str, function: Callable[..., Any], calls: Counter[str]) -> Callable[..., Any]:
    def counted(*args: Any, **kwargs: Any) -> Any:
        calls[name] += 1
        return function(*args, **kwargs)

    return counted


_EDGE_PROJECT: str = (
    'name = "assembly"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
    '[connection]\ndatabase = ":memory:"\n\n'
)
_EDGE_MODEL: str = (
    'MODEL (description "Orders", database "lake", schema "orders");\n\nSELECT 1 AS n\n'
)
_EDGE_SOURCE: str = (
    "sources:\n  - name: raw_orders\n    description: Raw orders.\n    managed: true\n"
    "    write_strategy: table\n{ns}    columns:\n      - name: n\n        type: INTEGER\n"
)
_EDGE_LOADER: str = (
    "from sqlbuild.loaders import loader\n\n\n"
    "@loader\ndef raw_orders(ctx):\n    '''Load raw orders.'''\n    return [{'n': 1}]\n"
)
_EDGE_VARS: dict[str, object] = {
    "tags": ["daily", "x"],
    "meta": {"b": 1, "a": "z"},
    "ratio": 1.5,
    "day": datetime.date(2024, 1, 2),
    "big": 10**30,
    "flag": True,
    "none": None,
    "mixed": [datetime.date(2024, 1, 2)],
    "name": "prod",
}


def _edge_seeds(*namespaces: str) -> str:
    return "seeds:\n" + "".join(
        f"  - name: s{index}\n    description: Seed {index}.\n{namespace}"
        "    columns:\n      - name: n\n        type: INTEGER\n"
        for index, namespace in enumerate(namespaces)
    )


def _edge_seed_files(*namespaces: str) -> dict[str, str]:
    return {
        "seeds/lookups.yml": _edge_seeds(*namespaces),
        "models/orders.sql": _EDGE_MODEL,
        **{f"seeds/s{index}.csv": "n\n1\n" for index in range(len(namespaces))},
    }


def _edge_target(text: str) -> str:
    return _EDGE_PROJECT + "[targets.dev]\n" + text


def _edge_source_files(ns: str = "") -> dict[str, str]:
    return {
        "sources/raw.yml": _EDGE_SOURCE.format(ns=ns),
        "python/loaders/raw.py": _EDGE_LOADER,
        "models/orders.sql": _EDGE_MODEL,
    }


# Projects whose assembly raises, or reads variables only Python renders: (description, files,
# effective vars merged over the project's).
EDGE_ASSEMBLY_PROJECTS: tuple[tuple[str, dict[str, str], dict[str, object] | None], ...] = (
    (
        "seed schema missing env",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${ENV:SQB_EDGE_UNSET}"\n'),
        },
        None,
    ),
    (
        "seed database unknown var",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    database: "${var:nope}"\n'),
        },
        None,
    ),
    (
        "seed unknown ctx",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${CTX:model.nope}"\n'),
        },
        None,
    ),
    (
        "seed template syntax",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${coalesce(}"\n'),
        },
        None,
    ),
    (
        "second seed fails after first",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "ok"\n', '    schema: "${ENV:SQB_EDGE_UNSET}"\n'),
        },
        None,
    ),
    (
        "preserved schema seed",
        {"sqlbuild_project.toml": _edge_target('schema = "preserve"\n'), **_edge_seed_files("")},
        None,
    ),
    (
        "preserved database and schema seed",
        {
            "sqlbuild_project.toml": _edge_target('database = "preserve"\nschema = "preserve"\n'),
            **_edge_seed_files(""),
        },
        None,
    ),
    (
        "preserved managed source",
        {
            "sqlbuild_project.toml": _edge_target('database = "preserve"\nschema = "dev"\n'),
            **_edge_source_files(),
        },
        None,
    ),
    (
        "managed source target missing env",
        {
            "sqlbuild_project.toml": _edge_target('schema = "${ENV:SQB_EDGE_UNSET}"\n'),
            **_edge_source_files(),
        },
        None,
    ),
    (
        "managed source loader schema unknown var",
        {
            "sqlbuild_project.toml": _edge_target(
                'schema = "dev"\nloader_schema = "${var:nope}"\n'
            ),
            **_edge_source_files(),
        },
        None,
    ),
    (
        "target schema override missing env",
        {
            "sqlbuild_project.toml": _edge_target('schema = "${ENV:SQB_EDGE_UNSET}"\n'),
            **_edge_seed_files(""),
        },
        None,
    ),
    (
        "target database override unknown var",
        {
            "sqlbuild_project.toml": _edge_target('database = "${var:nope}"\nschema = "dev"\n'),
            **_edge_seed_files(""),
        },
        None,
    ),
    (
        "default seed schema unknown var",
        {
            "sqlbuild_project.toml": _EDGE_PROJECT
            + '[defaults]\nseed_schema = "${var:nope}"\n\n[targets.dev]\nschema = "dev"\n',
            **_edge_seed_files(""),
        },
        None,
    ),
    (
        "default database missing env",
        {
            "sqlbuild_project.toml": _EDGE_PROJECT
            + '[defaults]\ndatabase = "${ENV:SQB_EDGE_UNSET}"\n\n[targets.dev]\nschema = "dev"\n',
            **_edge_seed_files(""),
        },
        None,
    ),
    (
        "model SQL the parser cannot read, then a seed error",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${ENV:SQB_EDGE_UNSET}"\n'),
            "models/orders.sql": 'MODEL (description "Orders");\n\nSELECT FROM WHERE (\n',
        },
        None,
    ),
    (
        "source error before seed error",
        {
            "sqlbuild_project.toml": _edge_target('schema = "${ENV:SQB_EDGE_UNSET}"\n'),
            **_edge_source_files(),
            "seeds/lookups.yml": _edge_seeds('    schema: "${var:nope}"\n'),
            "seeds/s0.csv": "n\n1\n",
        },
        None,
    ),
    (
        "list var whole",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${tags}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "list var in text",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "s_${tags}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "dict var whole",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${meta}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "dict var in text",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "s_${var:meta}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "float var whole and in text",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${ratio}"\n', '    schema: "s_${ratio}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "date var whole",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${day}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "date var in text",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "s_${day}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "big int var whole and in text",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${big}"\n', '    database: "d_${big}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "bool and none vars",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "${flag}"\n', '    schema: "s_${none}_${flag}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "list of dates in text",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files('    schema: "s_${mixed}"\n'),
        },
        _EDGE_VARS,
    ),
    (
        "list var in coalesce",
        {
            "sqlbuild_project.toml": _edge_target('schema = "dev"\n'),
            **_edge_seed_files("    schema: \"${coalesce(tags, 'x')}\"\n"),
        },
        _EDGE_VARS,
    ),
    (
        "dict var in target schema text",
        {"sqlbuild_project.toml": _edge_target('schema = "t_${meta}"\n'), **_edge_seed_files("")},
        _EDGE_VARS,
    ),
    (
        "float var in managed source target",
        {"sqlbuild_project.toml": _edge_target('schema = "${ratio}"\n'), **_edge_source_files()},
        _EDGE_VARS,
    ),
    (
        "unused opaque vars",
        {
            "sqlbuild_project.toml": _EDGE_PROJECT
            + '[vars]\nname = "prod"\n\n[targets.dev]\nschema = "dev_${name}"\n',
            **_edge_seed_files('    schema: "s_${name}"\n'),
            **_edge_source_files(),
        },
        _EDGE_VARS,
    ),
)
