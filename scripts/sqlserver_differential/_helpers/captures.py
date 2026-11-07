"""Read compile captures of SQL Server projects into harness cases."""

from __future__ import annotations

import json
from collections.abc import Iterator
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from scripts.sqlserver_differential.constants import (
    CAPTURE_KEY,
    COMPILED_KINDS,
    COMPILED_PROJECT_CAPTURE,
    SQLSERVER_DIALECT,
    NO_GATE,
    PARSE_MODE,
    SHARED_KEY,
    SHARED_NODES_KEY,
    VALIDATE_MODE,
)
from scripts.sqlserver_differential.exceptions import SqlServerDifferentialError

# Capture fields that hold SQL, per resource list: (field, kind, gate mode).
RESOURCE_FIELDS: dict[str, tuple[tuple[str, str, str], ...]] = {
    "sql_hook_files": (("sql_body", "hook", VALIDATE_MODE),),
    "audits": (
        ("sql_body", "audit", NO_GATE),
        ("measure_sql", "audit_measure", NO_GATE),
        ("evidence_sql", "audit_evidence", NO_GATE),
    ),
    "sql_tests": (("sql_body", "sql_test", NO_GATE),),
    "sql_scenarios": (("sql_body", "sql_scenario", NO_GATE),),
    "functions": (("body_sql", "function_body", PARSE_MODE),),
}
FUNCTIONS: str = "functions"
COMPILED_DIR: str = "project/target/compiled"
SCALAR_FUNCTION_KIND: str = "scalar_function_body"
HOOK_KEYS: tuple[str, ...] = ("pre_hooks", "post_hooks")
# The `language` enum of a function whose body is SQL; other languages' bodies are code.
SQL_LANGUAGE: str = "sqlbuild.compiler.compile.types:FunctionLanguage.SQL"
ENUM_KEY: str = "__enum__"


def scalar_function_ddl(function: dict[str, Any], body: str) -> str:
    """The SQL Server adapter's `CREATE OR ALTER FUNCTION` around a scalar function's body.

    Polyglot parses the bare body, as SQLBuild's gate does; SQL Server only ever receives it inside
    this statement, so that is the text SQLQuack parses. The adapter's own renderer
    (`SqlServerAdapter.render_create_function`) builds it, so the wrapping is SQLBuild's.
    """
    from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter

    destination: dict[str, Any] = function.get("destination") or {}
    name: Any = destination.get("qualified_name") or function.get("name")
    arguments: tuple[Any, ...] = tuple(
        SimpleNamespace(name=argument.get("name"), type=argument.get("type"))
        for argument in function.get("arguments") or []
    )
    statements: tuple[str, ...] = SqlServerAdapter().render_create_function(
        destination=str(name), arguments=arguments, returns=str(function.get("returns")),
        body_sql=body,
    )
    return ";\n".join(statements)


def capture_paths(work_dir: Path) -> list[Path]:
    """Every compiled-project capture under a compiler differential work dir."""
    paths: list[Path] = sorted(work_dir.glob(f"*/{COMPILED_PROJECT_CAPTURE}"))
    if not paths:
        raise SqlServerDifferentialError(f"no compile captures under {work_dir}")
    return paths


def load_capture(path: Path) -> dict[str, Any]:
    """The capture with its shared nodes inlined."""
    document: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    shared: dict[str, Any] = document[SHARED_NODES_KEY]

    def resolve(value: Any) -> Any:
        if isinstance(value, dict):
            if set(value) == {SHARED_KEY}:
                return resolve(shared[value[SHARED_KEY]])
            return {key: resolve(item) for key, item in value.items()}
        if isinstance(value, list):
            return [resolve(item) for item in value]
        return value

    return resolve(document[CAPTURE_KEY])


def hook_statements(entries: Any) -> Iterator[str]:
    """The SQL of a model's hook list: plain strings or entries with a `statement`."""
    for entry in entries or []:
        if isinstance(entry, str):
            yield entry
        elif isinstance(entry, dict) and isinstance(entry.get("statement"), str):
            yield entry["statement"]


def project_cases(*, path: Path, source: str) -> Iterator[dict[str, Any]]:
    """One case per SQL body of the captured project, when it compiled for SQL Server."""
    project: dict[str, Any] = load_capture(path)
    project_name: str = path.parents[2].name
    catalog: dict[str, Any] | None = project.get("binding_catalog")
    if catalog is None or catalog.get("dialect") != SQLSERVER_DIALECT:
        return

    def case(
        kind: str, name: str, sql: str, mode: str, placeholders: dict[str, str] | None = None
    ) -> dict[str, Any]:
        return {
            "id": f"{source}/{project_name}/{kind}/{name}",
            "source": source,
            "kind": kind,
            "sql": sql,
            "mode": mode,
            "placeholders": placeholders or {},
        }

    # Each model's own placeholder values, which SQLBuild substitutes for `@@@name`.
    model_placeholders: dict[str, dict[str, str]] = {}
    for model in project.get("models", []):
        values: dict[str, Any] = (model.get("config") or {}).get("values") or {}
        placeholders: dict[str, str] = {
            str(name): str(value) for name, value in (values.get("placeholders") or {}).items()
        }
        model_placeholders[model["name"]] = placeholders
        if isinstance(model.get("query_sql"), str):
            yield case("model", model["name"], model["query_sql"], VALIDATE_MODE, placeholders)
        for key in HOOK_KEYS:
            for index, statement in enumerate(hook_statements(values.get(key))):
                yield case(
                    "model_hook",
                    f"{model['name']}#{key}{index}",
                    statement,
                    VALIDATE_MODE,
                    placeholders,
                )
    for resources, fields in RESOURCE_FIELDS.items():
        for index, resource in enumerate(project.get(resources) or []):
            language: Any = resource.get("language")
            if isinstance(language, dict) and language.get(ENUM_KEY) != SQL_LANGUAGE:
                continue
            name: str = f"{resource.get('name')}#{index}"
            for field, kind, mode in fields:
                sql: Any = resource.get(field)
                if isinstance(sql, str) and sql.strip():
                    body: dict[str, Any] = case(kind, name, sql, mode)
                    if resources == FUNCTIONS and not resource.get("return_columns"):
                        body["kind"] = SCALAR_FUNCTION_KIND
                        body["sqlquack_sql"] = scalar_function_ddl(resource, sql)
                    yield body
    # The compiled tree the compile wrote: each model, audit and SQL test as the adapter runs it,
    # and each function's full DDL.
    compiled: Path = path.parents[2] / COMPILED_DIR
    for file in sorted(compiled.glob("**/*.sql")):
        sql: str = file.read_text(encoding="utf-8")
        if not sql.strip():
            continue
        relative: str = file.relative_to(compiled).as_posix()
        kind: str = COMPILED_KINDS.get(relative.split("/", 1)[0], "compiled")
        yield case(
            f"compiled_{kind}",
            relative,
            sql,
            VALIDATE_MODE if kind == "model" else NO_GATE,
            model_placeholders.get(file.stem) if kind == "model" else None,
        )
