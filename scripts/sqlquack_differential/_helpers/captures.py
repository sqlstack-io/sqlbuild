"""Read compile captures of Snowflake, DuckDB and SQLite projects into harness cases."""

from __future__ import annotations

import json
import re
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from scripts.sqlquack_differential.constants import (
    CAPTURE_KEY,
    COLUMN_REFERENCE,
    COMPILED_PROJECT_CAPTURE,
    ENUM_KEY,
    JOIN_KEYWORD,
    MEASURED_DIALECTS,
    MISSING_SUFFIX,
    MUTATION_SEPARATOR,
    OUTPUT_ALIAS_BEFORE,
    QUALIFIED_REFERENCE,
    REFERENCE_RESOURCE_TYPES,
    SHARED_KEY,
    SHARED_NODES_KEY,
    TABLE_FUNCTION_KIND,
    TABLE_FUNCTION_PREFIX,
    UNKNOWN_QUALIFIER,
    UNKNOWN_TYPE,
    UNORDERED_MAPPING_KEY,
)
from scripts.sqlquack_differential.exceptions import DifferentialError
from scripts.sqlquack_differential.types import Mutation


def capture_paths(work_dir: Path) -> list[Path]:
    """Every compiled-project capture under a compiler differential work dir."""
    paths: list[Path] = []
    for project in sorted(work_dir.iterdir()):
        capture: Path = project / COMPILED_PROJECT_CAPTURE
        if capture.is_file():
            paths.append(capture)
    if not paths:
        raise DifferentialError(f"no compile captures under {work_dir}")
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


def enum_member(value: dict[str, str]) -> str:
    """The member name of a captured enum value."""
    return value[ENUM_KEY].rsplit(".", 1)[-1]


def nullability_text(value: dict[str, str] | None) -> str:
    """`non_null`, `nullable` or `unknown`."""
    return "unknown" if value is None else enum_member(value).lower()


def project_cases(*, path: Path, source: str) -> Iterator[dict[str, Any]]:
    """One case per compiled model of the captured project compiled for a measured dialect."""
    project: dict[str, Any] = load_capture(path)
    project_name: str = path.parents[2].name
    catalog: dict[str, Any] | None = project["binding_catalog"]
    if catalog is None or catalog["dialect"] not in MEASURED_DIALECTS:
        return
    schemas: dict[str, dict[str, str]] = dict(catalog["schemas"][UNORDERED_MAPPING_KEY])
    shapes: dict[str, list[dict[str, Any]]] = dict(
        catalog["analysis_shapes"][UNORDERED_MAPPING_KEY]
    )
    functions: list[dict[str, str | None]] = [
        project_function(function) for function in project["functions"]
    ]
    for model in project["models"]:
        resources: dict[str, tuple[str, str]] = {}
        for reference in model["references"]:
            kind: str = enum_member(reference["ref_kind"])
            if kind not in REFERENCE_RESOURCE_TYPES:
                continue
            name: str = reference["ref_name"]
            relation: str = TABLE_FUNCTION_PREFIX + name if kind == TABLE_FUNCTION_KIND else name
            resources[relation] = (REFERENCE_RESOURCE_TYPES[kind], name)
        yield {
            "id": f"{source}/{project_name}/{model['name']}",
            "source": source,
            "dialect": catalog["dialect"],
            "quoted_ignore_case": catalog["quoted_ignore_case"],
            "query_sql": model["query_sql"],
            "placeholders": dict(model["config"]["values"].get("placeholders") or {}),
            "relations": relations(schemas=schemas, shapes=shapes, resources=resources),
            "functions": functions,
            "polyglot": polyglot_result(model),
        }


def project_function(function: dict[str, Any]) -> dict[str, str | None]:
    """A project function as SQLBuild types it for analysis: scalar ones by their return type."""
    returns: str | None = None if function["return_columns"] else function["returns"]
    return {"name": function["name"], "returns": returns}


def relations(
    *,
    schemas: dict[str, dict[str, str]],
    shapes: dict[str, list[dict[str, Any]]],
    resources: dict[str, tuple[str, str]],
) -> list[dict[str, Any]]:
    """Closed relations typed from `schemas`, then referenced relations with no closed shape."""
    result: list[dict[str, Any]] = []
    for name in sorted(set(schemas) | set(shapes)):
        shape_types, shape_nullability = shapes.get(name, [{}, {}])
        types: dict[str, str] = schemas.get(name) or shape_types
        if not types:
            continue
        resource_type, resource_name = resources.get(name, (None, None))
        result.append(
            {
                "name": name,
                "resource_type": resource_type,
                "resource_name": resource_name,
                "open": False,
                "columns": [
                    {
                        "name": column,
                        "data_type": None if data_type == UNKNOWN_TYPE else data_type,
                        "nullability": nullability_text(shape_nullability.get(column)),
                    }
                    for column, data_type in types.items()
                ],
            }
        )
    closed: set[str] = {relation["name"] for relation in result}
    for name, (resource_type, resource_name) in sorted(resources.items()):
        if name not in closed:
            result.append(
                {
                    "name": name,
                    "resource_type": resource_type,
                    "resource_name": resource_name,
                    "open": True,
                    "columns": [],
                }
            )
    return result


def polyglot_result(model: dict[str, Any]) -> dict[str, Any]:
    """The compile's inferred columns and lineage for the model."""
    inferred: list[dict[str, Any]] | None = model["inferred_columns"]
    columns: list[dict[str, Any]] | None = None
    if inferred is not None:
        columns = [inferred_column(column) for column in inferred]
    facts: list[dict[str, Any]] = model["fast_lineage_columns"] or []
    return {
        "columns": columns,
        "lineage": [lineage_column(fact) for fact in facts],
        "has_star": model["fast_lineage_has_star"],
    }


def inferred_column(column: dict[str, Any]) -> dict[str, Any]:
    """One inferred output column."""
    return {
        "name": column["name"],
        "data_type": column["type"],
        "nullability": nullability_text(column["nullability"]),
    }


def lineage_column(fact: dict[str, Any]) -> dict[str, Any]:
    """One output column's lineage fact."""
    sources: list[dict[str, str]] = [
        {
            "resource_type": enum_member(source["resource_type"]),
            "resource_name": source["resource_name"],
            "column": source["column_name"],
        }
        for source in fact["upstream_columns"]
    ]
    return {
        "output": fact["output_column"],
        "kind": enum_member(fact["transform_kind"]).lower(),
        "sources": sources,
    }


def mutated_cases(case: dict[str, Any]) -> Iterator[dict[str, Any]]:
    """Variants of `case` with one name broken (see `Mutation`), for name findings only."""
    sql: str = case["query_sql"]
    names: dict[str, int] = {}
    for relation in case["relations"]:
        for column in relation["columns"]:
            names[column["name"]] = names.get(column["name"], 0) + 1
    reference: re.Match[str] | None = column_reference(sql=sql, names=names)
    if reference is not None:
        start, end = reference.span(2)
        column: str = reference.group(2)
        renamed: str = sql[:start] + column + MISSING_SUFFIX + sql[end:]
        quoted: str = sql[:start] + f'"{column.upper()}"' + sql[end:]
        yield mutated(case=case, mutation=Mutation.UNKNOWN_COLUMN, sql=renamed)
        yield mutated(case=case, mutation=Mutation.QUOTED_UPPER, sql=quoted)
    qualified: list[re.Match[str]] = list(re.finditer(QUALIFIED_REFERENCE, sql))
    if qualified:
        start, end = qualified[0].span(1)
        requalified: str = sql[:start] + UNKNOWN_QUALIFIER + sql[end:]
        yield mutated(case=case, mutation=Mutation.UNKNOWN_QUALIFIER, sql=requalified)
    if not re.search(JOIN_KEYWORD, sql, re.IGNORECASE):
        return
    shared: list[re.Match[str]] = [match for match in qualified if names.get(match.group(2), 0) > 1]
    if shared:
        unqualified: str = sql[: shared[0].start(1)] + sql[shared[0].end(1) + 1 :]
        yield mutated(case=case, mutation=Mutation.UNQUALIFIED_SHARED, sql=unqualified)


def column_reference(*, sql: str, names: dict[str, int]) -> re.Match[str] | None:
    """The first reference to a catalog column that is not an output alias."""
    for match in re.finditer(COLUMN_REFERENCE, sql):
        before: str = sql[: match.start()]
        if match.group(2) in names and not re.search(OUTPUT_ALIAS_BEFORE, before, re.IGNORECASE):
            return match
    return None


def mutated(*, case: dict[str, Any], mutation: Mutation, sql: str) -> dict[str, Any]:
    """A copy of `case` with `sql` and no compile results."""
    return {
        "id": f"{case['id']}{MUTATION_SEPARATOR}{mutation}",
        "source": case["source"],
        "dialect": case["dialect"],
        "quoted_ignore_case": case["quoted_ignore_case"],
        "query_sql": sql,
        "placeholders": case["placeholders"],
        "relations": case["relations"],
        "functions": case["functions"],
        "polyglot": None,
    }
