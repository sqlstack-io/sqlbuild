"""Type each compiled model's outputs with GoogleSQL's analyzer, as BigQuery types them.

Usage: semantic_oracle.py CASES_JSONL RESULTS_JSONL OUT_JSONL EXECUTE_QUERY

For every case SQLQuack parsed (a mutated variant too, for its verdict), the normalized SQL gets a typed CTE per closed relation
it may read (`CAST(NULL AS <type>) AS <column>`) and a temporary function per project UDF stub,
and runs through `execute_query --mode=analyze --product_mode=external`. Each output column's
type comes from the resolved AST's output column list. Nullability is not measured: the analyzer
does not infer it. A case reading an open relation (no column list) is skipped.

Each line: {"id", "status": "ok" | "error" | "skipped", "detail", "outputs": [[name, type]]}.
"""

import concurrent.futures
import json
import re
import subprocess
import sys

UDF_PREFIX = "__sqlbuild_udf_"
OUTPUT = re.compile(r"^\W*\S+ AS (\S+) \[(.*)\]$")
# BigQuery's own type aliases, which GoogleSQL's analyzer does not know.
ALIAS_TYPES = {**dict.fromkeys(["INT", "SMALLINT", "INTEGER", "BIGINT", "TINYINT", "BYTEINT"], "INT64"),
               "DECIMAL": "NUMERIC", "BIGDECIMAL": "BIGNUMERIC"}
BIGQUERY_ALIASES = re.compile(r"(\bAS\s+)(INT|SMALLINT|INTEGER|BIGINT|TINYINT|BYTEINT|DECIMAL|BIGDECIMAL)\b", re.I)
TYPE_WORDS = [
    (r"^(INTEGER|INT|BIGINT|SMALLINT|TINYINT|BYTEINT|INT64)$", "INT64"),
    (r"^(DOUBLE|DOUBLE PRECISION|FLOAT|FLOAT64|REAL|FLOAT8)$", "FLOAT64"),
    (r"^(BOOLEAN|BOOL)$", "BOOL"),
    (r"^(VARCHAR|TEXT|CHAR|STRING|CHARACTER VARYING|NVARCHAR)(\(\d+\))?$", "STRING"),
    (r"^(DECIMAL|NUMERIC)(\(.*\))?$", "NUMERIC\\2"),
    (r"^(BIGDECIMAL|BIGNUMERIC)(\(.*\))?$", "BIGNUMERIC\\2"),
    (r"^(TIMESTAMPTZ|TIMESTAMP WITH TIME ZONE|TIMESTAMP_TZ|TIMESTAMP_LTZ)$", "TIMESTAMP"),
    (r"^(TIMESTAMP_NTZ|TIMESTAMP WITHOUT TIME ZONE)$", "DATETIME"),
]


def bigquery_type(spelling):
    """A catalog type spelled for BigQuery's CAST, or None when it has no BigQuery spelling."""
    text = " ".join(spelling.upper().split())
    for pattern, replacement in TYPE_WORDS:
        if re.match(pattern, text):
            return re.sub(pattern, replacement, text)
    if re.match(r"^(DATE|DATETIME|TIME|TIMESTAMP|BYTES|JSON|INTERVAL|GEOGRAPHY)$", text):
        return text
    if text.startswith(("ARRAY<", "STRUCT<")):
        return text
    return None


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
    """The analyzer program for `case`, with `typed` giving upstream models' analyzer types."""
    names = re.findall(r"[A-Za-z_]\w*", sql)
    used = set(names)
    ctes = []
    for relation in case["relations"]:
        if relation["name"] not in used:
            continue
        if relation["open"]:
            return None, f"open relation {relation['name']}"
        columns = []
        upstream = typed.get((relation["resource_type"], relation["resource_name"]))
        for column in relation["columns"]:
            if not column["declared"]:
                # An undeclared model column takes the analyzer's own type for that model.
                if upstream is None or column["name"] not in upstream:
                    return None, f"untyped upstream {relation['name']}.{column['name']}"
                column = {**column, "data_type": upstream[column["name"]]}
            spelled = bigquery_type(column["data_type"] or "")
            if spelled is None and column["data_type"] is None and column["name"] not in used and "*" not in sql:
                spelled = "BYTES"
            if spelled is None:
                return None, f"untyped column {relation['name']}.{column['name']} {column['data_type']}"
            columns.append(f"CAST(NULL AS {spelled}) AS `{column['name']}`")
        ctes.append(f"`{relation['name']}` AS (SELECT {', '.join(columns)})")
    functions = []
    for function in case["functions"]:
        stub = UDF_PREFIX + function["name"]
        if stub not in used or not function["returns"]:
            continue
        spelled = bigquery_type(function["returns"])
        if spelled is None:
            return None, f"untyped function {function['name']} {function['returns']}"
        count = argument_count(sql, stub)
        arguments = ", ".join(f"a{i} ANY TYPE" for i in range(count))
        functions.append(f"CREATE TEMP FUNCTION {stub}({arguments}) RETURNS {spelled} AS (CAST(NULL AS {spelled}));")
    body = re.sub(BIGQUERY_ALIASES, lambda m: m.group(1) + ALIAS_TYPES[m.group(2).upper()], sql.strip().rstrip(";"))
    # VARCHAR and DOUBLE are no BigQuery types; the model is not BigQuery SQL, and its casts are
    # typed as the nearest BigQuery type so its other outputs can still be judged.
    lenient = re.sub(r"(\bAS\s+)(VARCHAR|DOUBLE)\b", lambda m: m.group(1) + {"VARCHAR": "STRING", "DOUBLE": "FLOAT64"}[m.group(2).upper()], body, flags=re.I)
    marker = "\n-- lenient" if lenient != body else ""
    body = lenient
    if ctes:
        match = re.match(r"\s*WITH\s+(RECURSIVE\s+)?", body, re.I)
        if match:
            body = f"WITH {match.group(1) or ''}{', '.join(ctes)}, {body[match.end():]}"
        else:
            body = f"WITH {', '.join(ctes)} {body}"
    return "\n".join(functions + [body + ";" + marker]), None


def analyse(tool, item):
    case_id, text = item
    completed = subprocess.run(
        [tool, "--mode=analyze", "--product_mode=external", "--enabled_language_features=MAXIMUM", "-"],
        input=text,
        capture_output=True,
        text=True,
        cwd="/tmp",
    )
    output = completed.stdout + completed.stderr
    block = output.rsplit("QueryStmt", 1)
    if len(block) < 2 or completed.returncode != 0:
        return {"id": case_id, "status": "error", "detail": output.strip()[-400:], "outputs": []}
    outputs = []
    for line in block[1].splitlines()[1:]:
        if not re.match(r"^\W*\$?\w", line) or "output_column_list" in line:
            continue
        match = OUTPUT.match(line.strip())
        if not match:
            break
        outputs.append([match.group(1).strip("`"), match.group(2)])
    return {"id": case_id, "status": "ok", "detail": "lenient" if "-- lenient" in text else None, "outputs": outputs}


def main():
    cases_path, results_path, out_path, tool = sys.argv[1:5]
    cases = {case["id"]: case for case in map(json.loads, open(cases_path, encoding="utf-8"))}
    pending = [
        result
        for result in map(json.loads, open(results_path, encoding="utf-8"))
        if result["parse"] == "ok"
    ]
    # Upstream models are typed first: a model whose relations' columns the catalog leaves
    # untyped takes them from the analyzer's result for that model, in the same project.
    typed, rows, skipped = {}, [], []
    while pending:
        work, waiting = [], []
        for result in pending:
            case = cases[result["id"]]
            project = result["id"].split("#", 1)[0].rsplit("/", 1)[0]
            text, reason = program(case, result["sql"], typed.get(project, {}))
            if text is None:
                waiting.append((result, reason))
            else:
                work.append((result["id"], text))
        if not work:
            skipped = [{"id": r["id"], "status": "skipped", "detail": reason, "outputs": []} for r, reason in waiting]
            break
        with concurrent.futures.ThreadPoolExecutor(8) as pool:
            done = list(pool.map(lambda item: analyse(tool, item), work))
        for row in done:
            if row["status"] == "ok" and "#" not in row["id"]:
                project, model = row["id"].rsplit("/", 1)
                typed.setdefault(project, {})[("MODEL", model)] = dict(row["outputs"])
        rows += done
        pending = [result for result, _ in waiting]
    with open(out_path, "w", encoding="utf-8") as out:
        for row in rows + skipped:
            out.write(json.dumps(row) + "\n")
    counts = {}
    for row in rows + skipped:
        counts[row["status"]] = counts.get(row["status"], 0) + 1
    print(counts)


main()
