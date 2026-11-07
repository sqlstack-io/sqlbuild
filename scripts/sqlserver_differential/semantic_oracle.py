"""Type each case's outputs with SQL Server itself, as `sys.dm_exec_describe_first_result_set` reports them.

Usage: semantic_oracle.py CASES_JSONL RESULTS_JSONL OUT_JSONL

Connects to a local SQL Server as `sa` on localhost:1433, with the password in the environment
variable MSSQL_SA_PASSWORD. Each captured project gets a fresh database under the server's default
collation, which SQLBuild's SQL Server adapter keeps. Each relation a query names becomes a table with
the catalog's column types and NOT NULL where the catalog infers it, SQLBuild's logical spellings
taking the physical types the adapter gives them, and each project UDF stub becomes `__sqlbuild.udf_<name>` returning its declared type. The
query SQLQuack parsed is then described: the output columns' names, types and nullability are SQL
Server's, and a refusal is SQL Server's error number and message. An undeclared column of an
upstream model takes SQL Server's own type for that model, so the engine types the project end to
end. When SQL Server refuses the upstream model, its columns take the capture's catalog types, and
the case is marked `"upstream": "catalog"`. A case reading an open relation is skipped. Pass the
unchained cases file, so no fallback type comes from either side's inference.

Each line: {"id", "status": "ok" | "error" | "skipped", "detail", "outputs": [[name, type, nullable]]}.
"""

import json
import os
import re
import sys

import pymssql

UDF_PREFIX = "__sqlbuild.udf_"
UDF_SCHEMA = "__sqlbuild"
# SQLBuild's logical spellings SQL Server lacks or reads otherwise, as the adapter writes them
# (`render_loader_logical_type`, `FrameworkType`): TIMESTAMP is rowversion in SQL Server.
PHYSICAL = {
    "DOUBLE": "FLOAT",
    "STRING": "NVARCHAR(MAX)",
    "BOOLEAN": "BIT",
    "JSON": "NVARCHAR(MAX)",
    "TIMESTAMP": "DATETIME2",
    "VARCHAR": "VARCHAR(MAX)",
    "NVARCHAR": "NVARCHAR(MAX)",
}
UNTYPED_UNUSED = "NVARCHAR(MAX)"


def physical(spelling):
    return PHYSICAL.get(spelling.strip().upper(), spelling)


def bracket(name):
    return "[" + name.replace("]", "]]") + "]"


def qualified(name):
    return ".".join(bracket(part) for part in name.split("."))


def argument_count(sql, stub):
    """The number of top-level arguments of the first call of `stub` in `sql`."""
    start = re.search(re.escape(stub) + r"\s*\(", sql, re.I).end()
    depth, count, empty = 0, 1, True
    for char in sql[start:]:
        if char == "(":
            depth += 1
        elif char == ")":
            if depth == 0:
                break
            depth -= 1
        elif char == "," and depth == 0:
            count += 1
        if not char.isspace():
            empty = False if char != ")" or depth else empty
    return 0 if empty else count


class Project:
    """One project's database: the tables and stubs created so far."""

    def __init__(self, connection, database):
        self.cursor = connection.cursor()
        self.database = database
        self.created = set()
        self.refused = set()
        self.fallback = set()
        self.cursor.execute(f"IF DB_ID('{database}') IS NOT NULL DROP DATABASE {bracket(database)}")
        self.cursor.execute(f"CREATE DATABASE {bracket(database)}")
        self.cursor.execute(f"USE {bracket(database)}")
        self.cursor.execute(f"EXEC('CREATE SCHEMA {bracket(UDF_SCHEMA)}')")

    def prepare(self, case, sql, typed):
        """Create what `sql` reads; the reason it cannot be typed yet, or None."""
        lowered = sql.lower()
        used = {word.lower() for word in re.findall(r"[A-Za-z_][\w$#@]*", sql)}
        for relation in case["relations"]:
            name = relation["name"]
            if name.split(".")[-1].lower() not in used or name in self.created:
                continue
            if relation["open"]:
                return f"open relation {name}"
            upstream = typed.get(relation["resource_name"]) if relation["resource_type"] == "MODEL" else None
            columns = []
            for column in relation["columns"]:
                data_type = column["data_type"]
                not_null = column["nullability"] == "non_null"
                if not column["declared"]:
                    if upstream is not None and column["name"].lower() in upstream:
                        data_type, nullable = upstream[column["name"].lower()]
                        not_null = not nullable
                    elif relation["resource_name"] in self.refused:
                        self.fallback.add(case["id"])
                    else:
                        return f"untyped upstream {name}.{column['name']}"
                if data_type is None:
                    if column["name"].lower() in used or "*" in sql:
                        return f"untyped column {name}.{column['name']}"
                    data_type = UNTYPED_UNUSED
                columns.append(f"{bracket(column['name'])} {physical(data_type)}{' NOT NULL' if not_null else ''}")
            parts = name.split(".")
            if len(parts) == 2:
                self.cursor.execute(f"IF SCHEMA_ID('{parts[0]}') IS NULL EXEC('CREATE SCHEMA {bracket(parts[0])}')")
            self.cursor.execute(f"CREATE TABLE {qualified(name)} ({', '.join(columns)})")
            self.created.add(name)
        for function in case["functions"]:
            stub = UDF_PREFIX + function["name"]
            if stub.lower() not in lowered or not function["returns"] or stub in self.created:
                continue
            count = argument_count(sql, stub)
            parameters = ", ".join(f"@a{i} NVARCHAR(MAX)" for i in range(count))
            returns = physical(function["returns"])
            self.cursor.execute(
                f"EXEC('CREATE FUNCTION {UDF_SCHEMA}.{bracket('udf_' + function['name'])} ({parameters}) "
                f"RETURNS {returns} AS BEGIN RETURN NULL END')"
            )
            self.created.add(stub)
        return None

    def describe(self, case_id, sql):
        body = sql.strip().rstrip(";").strip()
        self.cursor.execute(
            "SELECT name, system_type_name, is_nullable, error_number, error_message "
            "FROM sys.dm_exec_describe_first_result_set(%s, NULL, 0) ORDER BY column_ordinal",
            (body,),
        )
        rows = self.cursor.fetchall()
        failed = [row for row in rows if row[3] is not None]
        if failed:
            number, message = failed[0][3], failed[0][4]
            return {"id": case_id, "status": "error", "detail": f"{number}: {message[:300]}", "outputs": []}
        outputs = [[name, data_type, bool(nullable)] for name, data_type, nullable, _, _ in rows]
        return {"id": case_id, "status": "ok", "detail": None, "outputs": outputs}


def main():
    cases_path, results_path, out_path = sys.argv[1:4]
    connection = pymssql.connect("localhost", "sa", os.environ["MSSQL_SA_PASSWORD"], autocommit=True)
    cases = {case["id"]: case for case in map(json.loads, open(cases_path, encoding="utf-8"))}
    results = [r for r in map(json.loads, open(results_path, encoding="utf-8")) if r["sql"]]
    by_project = {}
    for result in results:
        by_project.setdefault(result["id"].split("#", 1)[0].rsplit("/", 1)[0], []).append(result)
    rows = []
    for index, (project_id, pending) in enumerate(sorted(by_project.items())):
        project = Project(connection, f"sqdiff_{index}")
        typed = {}
        while pending:
            waiting, progressed = [], False
            for result in pending:
                reason = project.prepare(cases[result["id"]], result["sql"], typed)
                if reason is not None:
                    waiting.append((result, reason))
                    continue
                progressed = True
                row = project.describe(result["id"], result["sql"])
                row["upstream"] = "catalog" if result["id"] in project.fallback else "engine"
                rows.append(row)
                if "#" not in row["id"]:
                    model = row["id"].rsplit("/", 1)[1]
                    if row["status"] == "ok":
                        typed[model] = {name.lower(): (data_type, nullable) for name, data_type, nullable in row["outputs"]}
                    else:
                        project.refused.add(model)
            pending = [result for result, _ in waiting]
            if not progressed:
                rows += [{"id": r["id"], "status": "skipped", "detail": why, "outputs": []} for r, why in waiting]
                break
        project.cursor.execute("USE master")
        project.cursor.execute(f"DROP DATABASE {bracket(project.database)}")
    with open(out_path, "w", encoding="utf-8") as out:
        for row in sorted(rows, key=lambda row: row["id"]):
            out.write(json.dumps(row) + "\n")
    counts = {}
    for row in rows:
        counts[row["status"]] = counts.get(row["status"], 0) + 1
    print(counts)


main()
