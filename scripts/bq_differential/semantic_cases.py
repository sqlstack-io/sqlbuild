"""Write the BigQuery semantic differential's cases from compile captures.

Usage: semantic_cases.py OUT LABEL=WORK_DIR ...

One case per model of each captured project whose binding catalog is BigQuery, with the
relations and functions the compile's binding catalog knew and what the compile recorded
(inferred columns and fast lineage). Each model also yields variants with one name broken, which
measure name findings only: an unknown column, the column in upper case and backquotes (which
BigQuery resolves to the same column), an unknown qualifier, and an unqualified column two joined
relations share.
"""

import json
import re
import sys
from pathlib import Path

CAPTURE = "captures/0-compile/003-compiled_project.json"
DIALECT = "bigquery"
UNKNOWN_TYPE = "UNKNOWN"
UNORDERED = "__unordered_mapping__"
RESOURCE_TYPES = {"REF": "MODEL", "SOURCE": "SOURCE", "SEED": "SEED", "TABLE_FUNCTION": "TABLE_FN"}
TABLE_FUNCTION_PREFIX = "__sqlbuild_table_function_"
MISSING_SUFFIX = "_missing"
UNKNOWN_QUALIFIER = "zq"
COLUMN_REFERENCE = r"(?<![\w\"'`])(?:([a-z]\w*)\.)?([a-z]\w*)(?![\w\"`(.])"
QUALIFIED_REFERENCE = r"(?<![\w\"'`.])([a-z]\w*)\.([a-z]\w*)\b"
OUTPUT_ALIAS_BEFORE = r"\bAS\s*$"
JOIN_KEYWORD = r"\bJOIN\b"


def load(path):
    document = json.load(open(path, encoding="utf-8"))
    shared = document["__shared_nodes__"]

    def resolve(value):
        if isinstance(value, dict):
            if set(value) == {"__shared__"}:
                return resolve(shared[value["__shared__"]])
            return {key: resolve(item) for key, item in value.items()}
        if isinstance(value, list):
            return [resolve(item) for item in value]
        return value

    return resolve(document["__capture__"])


def member(value):
    return value["__enum__"].rsplit(".", 1)[-1]


def nullability(value):
    return "unknown" if value is None else member(value).lower()


def project_cases(path, source):
    project = load(path)
    catalog = project["binding_catalog"]
    if catalog is None or catalog["dialect"] != DIALECT:
        return
    name = Path(path).parents[2].name
    schemas = dict(catalog["schemas"][UNORDERED])
    shapes = dict(catalog["analysis_shapes"][UNORDERED])
    functions = [
        {"name": f["name"], "returns": None if f["return_columns"] else f["returns"]}
        for f in project["functions"]
    ]
    declared = {
        model["name"]: {
            column["name"]: column["type"]
            for column in ((model.get("schema_entry") or {}).get("columns") or [])
            if column.get("type")
        }
        for model in project["models"]
    }
    for model in project["models"]:
        resources = {}
        for reference in model["references"]:
            kind = member(reference["ref_kind"])
            if kind not in RESOURCE_TYPES:
                continue
            ref = reference["ref_name"]
            relation = TABLE_FUNCTION_PREFIX + ref if kind == "TABLE_FUNCTION" else ref
            resources[relation] = (RESOURCE_TYPES[kind], ref)
        values = (model.get("config") or {}).get("values") or {}
        placeholders = {str(k): str(v) for k, v in (values.get("placeholders") or {}).items()}
        inferred = model["inferred_columns"]
        yield {
            "id": f"{source}/{name}/{model['name']}",
            "source": source,
            "query_sql": model["query_sql"],
            "placeholders": placeholders,
            "binding_validated": model["binding_validated"],
            "relations": relations(schemas, shapes, resources, declared),
            "functions": functions,
            "polyglot": {
                "columns": None
                if inferred is None
                else [
                    {"name": c["name"], "data_type": c["type"], "nullability": nullability(c["nullability"])}
                    for c in inferred
                ],
                "lineage": [
                    {
                        "output": fact["output_column"],
                        "kind": member(fact["transform_kind"]).lower(),
                        "sources": [
                            {
                                "resource_type": member(s["resource_type"]),
                                "resource_name": s["resource_name"],
                                "column": s["column_name"],
                            }
                            for s in fact["upstream_columns"]
                        ],
                    }
                    for fact in model["fast_lineage_columns"] or []
                ],
                "has_star": model["fast_lineage_has_star"],
            },
        }


def relations(schemas, shapes, resources, declared):
    result = []
    for name in sorted(set(schemas) | set(shapes)):
        shape_types, shape_nullability = shapes.get(name, [{}, {}])
        types = schemas.get(name) or shape_types
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
                        "nullability": nullability(shape_nullability.get(column)),
                        "declared": resource_type != "MODEL" or column in declared.get(resource_name, {}),
                    }
                    for column, data_type in types.items()
                ],
            }
        )
    closed = {relation["name"] for relation in result}
    for name, (resource_type, resource_name) in sorted(resources.items()):
        if name not in closed:
            result.append(
                {"name": name, "resource_type": resource_type, "resource_name": resource_name, "open": True, "columns": []}
            )
    return result


def mutated_cases(case):
    sql = case["query_sql"]
    names = {}
    for relation in case["relations"]:
        for column in relation["columns"]:
            names[column["name"]] = names.get(column["name"], 0) + 1
    reference = None
    for match in re.finditer(COLUMN_REFERENCE, sql):
        if match.group(2) in names and not re.search(OUTPUT_ALIAS_BEFORE, sql[: match.start()], re.I):
            reference = match
            break
    if reference is not None:
        start, end = reference.span(2)
        column = reference.group(2)
        yield mutated(case, "unknown_column", sql[:start] + column + MISSING_SUFFIX + sql[end:])
        yield mutated(case, "quoted_upper", sql[:start] + f"`{column.upper()}`" + sql[end:])
    qualified = list(re.finditer(QUALIFIED_REFERENCE, sql))
    if qualified:
        start, end = qualified[0].span(1)
        yield mutated(case, "unknown_qualifier", sql[:start] + UNKNOWN_QUALIFIER + sql[end:])
    if not re.search(JOIN_KEYWORD, sql, re.I):
        return
    shared = [match for match in qualified if names.get(match.group(2), 0) > 1]
    if shared:
        yield mutated(case, "unqualified_shared", sql[: shared[0].start(1)] + sql[shared[0].end(1) + 1 :])


def mutated(case, mutation, sql):
    return {**case, "id": f"{case['id']}#{mutation}", "query_sql": sql, "polyglot": None}


def main():
    out = Path(sys.argv[1])
    count = {"compiled": 0, "mutated": 0}
    with out.open("w", encoding="utf-8") as handle:
        for item in sys.argv[2:]:
            source, root = item.split("=", 1)
            for path in sorted(Path(root).glob(f"*/{CAPTURE}")):
                for case in project_cases(path, source):
                    handle.write(json.dumps(case) + "\n")
                    count["compiled"] += 1
                    for variant in mutated_cases(case):
                        handle.write(json.dumps(variant) + "\n")
                        count["mutated"] += 1
    print(count)


main()
