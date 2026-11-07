"""Type each case's outputs with ClickHouse itself, as `DESCRIBE (query)` reports them.

Usage: semantic_oracle.py CASES_JSONL RESULTS_JSONL OUT_JSONL CLICKHOUSE [JOBS]

For every case SQLQuack parsed (a mutated variant too, for its verdict), a fresh `clickhouse local`
session under SQLBuild's ClickHouse session settings creates each relation the query names as a
Memory table with the catalog's column types and each project UDF stub as a SQL function returning
`Nullable(<type>)`, as SQLBuild's adapter defines it, then runs `DESCRIBE (<query>)`. The output
columns' names and types, nullability included, are ClickHouse's; an error is ClickHouse's first
refusal, with its code name. A case reading an open relation (no column list) is skipped. An
undeclared column of an upstream model takes ClickHouse's own type for that model, so ClickHouse
types the project end to end.

Each line: {"id", "status": "ok" | "error" | "skipped", "detail", "outputs": [[name, type]]}.
"""

import concurrent.futures
import json
import re
import subprocess
import sys

UDF_PREFIX = "__sqlbuild_udf_"
# The session settings SQLBuild's ClickHouse adapter requires (`adapters/clickhouse/constants.py`),
# which change how ClickHouse types a query (`join_use_nulls`, `cast_keep_nullable`).
SESSION_SETTINGS = (
    "SET join_use_nulls = 1, cast_keep_nullable = 1, union_default_mode = 'DISTINCT', "
    "except_default_mode = 'DISTINCT', intersect_default_mode = 'DISTINCT', "
    "date_time_input_format = 'best_effort', cast_string_to_date_time_mode = 'best_effort', "
    "insert_null_as_default = 0"
)
UNTYPED_UNUSED = "String"
ERROR = re.compile(r"Code: (\d+)\. DB::Exception: (.*?)(?: \(([A-Z_]+)\))?(?: \(version [^)]*\))?$", re.M)


def quoted(name):
    return "`" + name.replace("\\", "\\\\").replace("`", "\\`") + "`"


def argument_count(sql, stub):
    """The number of top-level arguments of the first call of `stub` in `sql`."""
    start = re.search(rf"{stub}\s*\(", sql).end()
    depth, count, empty = 0, 1, True
    for char in sql[start:]:
        if char in "([":
            depth += 1
        elif char in ")]":
            if depth == 0:
                break
            depth -= 1
        elif char == "," and depth == 0:
            count += 1
        if not char.isspace():
            empty = False if char != ")" or depth else empty
    return 0 if empty else count


def program(case, sql, typed):
    """The clickhouse-local program for `case`, with `typed` giving upstream models' types."""
    used = set(re.findall(r"[A-Za-z_]\w*", sql))
    statements = [SESSION_SETTINGS]
    for relation in case["relations"]:
        if relation["name"] not in used:
            continue
        if relation["open"]:
            return None, f"open relation {relation['name']}"
        upstream = typed.get((relation["resource_type"], relation["resource_name"]))
        columns = []
        for column in relation["columns"]:
            data_type = column["data_type"]
            if not column["declared"]:
                if upstream is None or column["name"] not in upstream:
                    return None, f"untyped upstream {relation['name']}.{column['name']}"
                data_type = upstream[column["name"]]
            if data_type is None:
                if column["name"] in used or "*" in sql:
                    return None, f"untyped column {relation['name']}.{column['name']}"
                data_type = UNTYPED_UNUSED
            columns.append(f"{quoted(column['name'])} {data_type}")
        statements.append(f"CREATE TABLE {quoted(relation['name'])} ({', '.join(columns)}) ENGINE = Memory")
    for function in case["functions"]:
        stub = UDF_PREFIX + function["name"]
        if stub not in used or not function["returns"]:
            continue
        count = argument_count(sql, stub)
        arguments = ", ".join(f"a{i}" for i in range(count))
        statements.append(f"CREATE FUNCTION {stub} AS ({arguments}) -> CAST(NULL AS Nullable({function['returns']}))")
    body = sql.strip().rstrip(";").strip()
    statements.append(f"DESCRIBE ({body}) FORMAT JSONEachRow")
    return ";\n".join(statements) + ";\n", None


def analyse(tool, item):
    case_id, text = item
    completed = subprocess.run([tool, "local", "-n"], input=text, capture_output=True, text=True, cwd="/tmp")
    if completed.returncode != 0:
        match = ERROR.search(completed.stderr)
        detail = (
            f"{match.group(3) or match.group(1)}: {match.group(2)[:300]}" if match else completed.stderr.strip()[-300:]
        )
        return {"id": case_id, "status": "error", "detail": detail, "outputs": []}
    outputs = [[row["name"], row["type"]] for row in map(json.loads, completed.stdout.splitlines())]
    return {"id": case_id, "status": "ok", "detail": None, "outputs": outputs}


def main():
    cases_path, results_path, out_path, tool = sys.argv[1:5]
    jobs = int(sys.argv[5]) if len(sys.argv) > 5 else 6
    cases = {case["id"]: case for case in map(json.loads, open(cases_path, encoding="utf-8"))}
    pending = [r for r in map(json.loads, open(results_path, encoding="utf-8")) if r["parse"] == "ok"]
    typed, rows, skipped = {}, [], []
    while pending:
        work, waiting = [], []
        for result in pending:
            project = result["id"].split("#", 1)[0].rsplit("/", 1)[0]
            text, reason = program(cases[result["id"]], result["sql"], typed.get(project, {}))
            if text is None:
                waiting.append((result, reason))
            else:
                work.append((result["id"], text))
        if not work:
            skipped = [{"id": r["id"], "status": "skipped", "detail": reason, "outputs": []} for r, reason in waiting]
            break
        with concurrent.futures.ThreadPoolExecutor(jobs) as pool:
            done = list(pool.map(lambda item: analyse(tool, item), work))
        for row in done:
            if row["status"] == "ok" and "#" not in row["id"]:
                project, model = row["id"].rsplit("/", 1)
                typed.setdefault(project, {})[("MODEL", model)] = dict(row["outputs"])
        rows += done
        pending = [result for result, _ in waiting]
    with open(out_path, "w", encoding="utf-8") as out:
        for row in sorted(rows + skipped, key=lambda row: row["id"]):
            out.write(json.dumps(row) + "\n")
    counts = {}
    for row in rows + skipped:
        counts[row["status"]] = counts.get(row["status"], 0) + 1
    print(counts)


main()
