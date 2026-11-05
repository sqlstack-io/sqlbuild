"""SQL that checks for and drops the warehouse objects a DuckShop build creates.

DuckShop may be built into a schema that holds other objects, so cleanup must name exactly what the
project creates rather than drop the schema or archive whatever looks stale. The list comes from
`sqb compile --json`, which already reflects the target in sqlbuild_local.toml:

    sqb compile --json > target/duckshop-compile.json
    python ../scripts/relations.py check target/duckshop-compile.json > target/check.sql
    python ../scripts/relations.py drop target/duckshop-compile.json > target/drop.sql

`check` lists tables, views and functions that already exist under a DuckShop name, and any
SQLBuild state tables (`_sqlbuild_*`) already in the schema. `drop` is one Snowflake Scripting
block that drops every model, snapshot, seed and source table and every function the project
creates, including functions a SQL hook creates, which the compile output does not list.
SQLBuild's own state tables (`_sqlbuild_*`) are left alone, since other projects in the same schema
may share them.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

PROJECT_DIR = Path(__file__).resolve().parents[1] / "project"


def _function_arguments(relative_path: str) -> list[str]:
    text = (PROJECT_DIR / relative_path).read_text()
    if relative_path.endswith(".py"):
        match = re.search(r"arguments=\{([^}]*)\}", text)
        return re.findall(r'"\w+":\s*"([^"]+)"', match.group(1)) if match else []
    match = re.search(r"arguments \(([^)]*)\)", text)
    if not match:
        return []
    return [part.strip().split(None, 1)[1] for part in match.group(1).split(",") if part.strip()]


def managed_sources() -> list[str]:
    """Managed source names from sources/*.yml; each loader writes a table of the same name."""

    names: list[str] = []
    for path in sorted((PROJECT_DIR / "sources").glob("*.yml")):
        for block in re.split(r"\n  - name: ", path.read_text())[1:]:
            if re.search(r"^    managed: true$", block, flags=re.MULTILINE):
                names.append(block.split("\n", 1)[0].strip())
    return names


HOOK_FUNCTION = re.compile(
    r"CREATE\s+OR\s+REPLACE\s+FUNCTION\s+\S*?\.(\w+)\s*\(([^)]*)\)", flags=re.IGNORECASE
)


def hook_functions() -> list[str]:
    """`name(type, ...)` of each function a SQL hook creates; SQLBuild does not declare them."""

    found: list[str] = []
    for path in sorted(PROJECT_DIR.glob("models/**/_hooks/sql/*.sql")):
        for name, arguments in HOOK_FUNCTION.findall(path.read_text()):
            types = [
                part.strip().split(None, 1)[1] for part in arguments.split(",") if part.strip()
            ]
            found.append(f"{name}({', '.join(types)})")
    return found


def objects(compile_json: dict[str, object]) -> tuple[str, list[tuple[str, str]]]:
    """The schema-qualified prefix and each (object kind, name or signature) the project creates."""

    resources = compile_json["resources"]
    functions = resources["functions"]  # type: ignore[index]
    if not functions:
        raise SystemExit(
            "compile output has no functions to read the target database and schema from"
        )
    database, schema, _ = str(functions[0]["qualified_name"]).split(".")  # type: ignore[index]
    found: list[tuple[str, str]] = []
    for model in resources["models"]:  # type: ignore[index]
        kind = "VIEW" if model["materialized"] == "view" else "TABLE"
        found.append((kind, model["name"]))
    for seed in resources["seeds"]:  # type: ignore[index]
        found.append(("TABLE", seed["name"]))
    for name in managed_sources():
        found.append(("TABLE", name))
    for function in functions:  # type: ignore[union-attr]
        arguments = ", ".join(_function_arguments(function["relative_path"]))
        found.append(("FUNCTION", f"{function['name']}({arguments})"))
    for signature in hook_functions():
        found.append(("FUNCTION", signature))
    return f"{database}.{schema}", found


def check_sql(prefix: str, found: list[tuple[str, str]]) -> str:
    database, schema = prefix.split(".")
    names = sorted({name.split("(")[0].upper() for kind, name in found if kind != "FUNCTION"})
    functions = sorted({name.split("(")[0].upper() for kind, name in found if kind == "FUNCTION"})
    listed = ",\n    ".join(f"'{name}'" for name in names)
    listed_functions = ",\n    ".join(f"'{name}'" for name in functions)
    return (
        "SELECT table_name AS object_name, table_type AS object_type, created, last_altered\n"
        f"FROM {database}.INFORMATION_SCHEMA.TABLES\n"
        f"WHERE table_schema = '{schema.upper()}'\n"
        f"  AND (\n    table_name IN (\n    {listed}\n    )\n"
        "    OR STARTSWITH(table_name, '_SQLBUILD_')\n  )\n"
        "UNION ALL\n"
        "SELECT function_name, 'FUNCTION', created, last_altered\n"
        f"FROM {database}.INFORMATION_SCHEMA.FUNCTIONS\n"
        f"WHERE function_schema = '{schema.upper()}'\n"
        f"  AND function_name IN (\n    {listed_functions}\n  )\n"
        "ORDER BY object_name"
    )


def drop_sql(prefix: str, found: list[tuple[str, str]]) -> str:
    order = {"VIEW": 0, "TABLE": 1, "FUNCTION": 2}
    lines = [
        f"  DROP {kind} IF EXISTS {prefix}.{name};"
        for kind, name in sorted(found, key=lambda item: (order[item[0]], item[1]))
    ]
    return "EXECUTE IMMEDIATE $$\nBEGIN\n" + "\n".join(lines) + "\nEND;\n$$"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("action", choices=["check", "drop"])
    parser.add_argument("compile_json", type=Path, help="output of `sqb compile --json`")
    arguments = parser.parse_args(argv)
    prefix, found = objects(json.loads(arguments.compile_json.read_text()))
    print(check_sql(prefix, found) if arguments.action == "check" else drop_sql(prefix, found))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
