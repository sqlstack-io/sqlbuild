"""Judge each compiled output's type three ways: Polyglot, SQLQuack and SQL Server's description.

Usage: semantic_types.py CASES_JSONL RESULTS_JSONL ORACLE_JSONL OUT_JSON

Every type spelling either side gives is canonicalized by SQL Server itself, as the type it gives
`CAST(NULL AS <spelling>)`, SQLBuild's logical spellings (DOUBLE, STRING, BOOLEAN, TIMESTAMP...)
taking the physical types the adapter writes for them. A spelling SQL Server refuses is kept as
written and cannot match. The type class compares the base type with its parameters dropped;
`params` counts outputs whose base types agree and whose parameters differ, and nullability is
judged on its own against the engine's `is_nullable`. Uses the same connection as semantic_oracle.py.

Type classes per output: `agree` (all three), `polyglot_wrong` (SQLQuack matches SQL Server),
`sqlquack_wrong` (Polyglot matches), `both_wrong`, `sqlquack_untyped`,
`sqlquack_untyped_polyglot_wrong`, `polyglot_untyped_quack_right`, `polyglot_untyped_quack_wrong`,
`both_untyped`. Outputs of cases SQL Server could not type are counted by its error.
"""

import collections
import json
import os
import re
import sys

import pymssql

PHYSICAL = {
    "DOUBLE": "FLOAT",
    "STRING": "NVARCHAR(MAX)",
    "BOOLEAN": "BIT",
    "JSON": "NVARCHAR(MAX)",
    "TIMESTAMP": "DATETIME2",
}


def canonical_names(spellings):
    """SQL Server's own name for each spelling it accepts."""
    connection = pymssql.connect("localhost", "sa", os.environ["MSSQL_SA_PASSWORD"], autocommit=True)
    cursor = connection.cursor()
    names = {}
    for spelling in spellings:
        written = PHYSICAL.get(spelling.strip().upper(), spelling)
        cursor.execute(
            "SELECT system_type_name, error_number FROM sys.dm_exec_describe_first_result_set(%s, NULL, 0)",
            (f"SELECT CAST(NULL AS {written}) AS x",),
        )
        rows = cursor.fetchall()
        if rows and rows[0][1] is None:
            names[spelling] = rows[0][0]
    return names


def split(name):
    """`(base type, full type)` of a canonical SQL Server name."""
    return re.sub(r"\(.*\)$", "", name).strip().lower(), name.lower().replace(" ", "")


def main():
    cases_path, results_path, oracle_path, out_path = sys.argv[1:5]
    cases = {case["id"]: case for case in map(json.loads, open(cases_path, encoding="utf-8"))}
    results = {r["id"]: r for r in map(json.loads, open(results_path, encoding="utf-8"))}
    oracle = {r["id"]: r for r in map(json.loads, open(oracle_path, encoding="utf-8"))}
    spellings = set()
    for case_id, result in results.items():
        if "#" in case_id or result["parse"] != "ok" or cases[case_id]["polyglot"]["columns"] is None:
            continue
        spellings |= {c["data_type"] for c in cases[case_id]["polyglot"]["columns"] if c["data_type"]}
        spellings |= {t for _, t, _ in result["sqlquack_outputs"] if t}
    names = canonical_names(sorted(spellings))

    def canon(spelling):
        if not spelling or spelling.upper() in ("UNKNOWN", "NULL"):
            return None
        return split(names.get(spelling, spelling))

    classes = collections.Counter()
    params = collections.Counter()
    nulls = collections.Counter()
    by_source = collections.defaultdict(collections.Counter)
    patterns = collections.defaultdict(list)
    for case_id, result in results.items():
        if "#" in case_id or result["parse"] != "ok":
            continue
        polyglot = cases[case_id]["polyglot"]["columns"]
        if polyglot is None:
            continue
        source = cases[case_id]["source"]
        verdict = oracle.get(case_id, {"status": "missing", "detail": ""})
        poly = {c["name"].lower(): c for c in polyglot}
        quack = {name.lower(): (data_type, nullability) for name, data_type, nullability in result["sqlquack_outputs"]}
        if verdict.get("upstream") == "catalog":
            classes["catalog_upstream"] += len(quack)
            by_source[source]["catalog_upstream"] += len(quack)
            continue
        if verdict["status"] != "ok":
            label = f"engine_{verdict['status']}: " + (verdict.get("detail") or "").split(":")[0][:60]
            classes[label] += len(quack)
            by_source[source][label] += len(quack)
            continue
        engine = {name.lower(): (split(data_type), nullable) for name, data_type, nullable in verdict["outputs"]}
        for name, (quack_type, quack_null) in quack.items():
            column = poly.get(name, {})
            p, q = canon(column.get("data_type")), canon(quack_type)
            g, g_null = engine.get(name, (None, None))
            pb, qb, gb = (p or [None])[0], (q or [None])[0], (g or [None])[0]
            if g is None:
                label = "engine_unnamed"
            elif pb == gb and qb == gb:
                label = "agree"
            elif qb is None and pb is None:
                label = "both_untyped"
            elif qb is None:
                label = "sqlquack_untyped" if pb == gb else "sqlquack_untyped_polyglot_wrong"
            elif pb is None:
                label = "polyglot_untyped_quack_right" if qb == gb else "polyglot_untyped_quack_wrong"
            elif qb == gb:
                label = "polyglot_wrong"
            elif pb == gb:
                label = "sqlquack_wrong"
            else:
                label = "both_wrong"
            classes[label] += 1
            by_source[source][label] += 1
            if label != "agree":
                patterns[(label, p and p[1], q and q[1], g and g[1])].append(f"{case_id}::{name}")
            if g is None:
                continue
            for side, typed in (("polyglot", p), ("sqlquack", q)):
                if typed and typed[0] == g[0] and typed[1] != g[1]:
                    params[side] += 1
                    patterns[(f"{side}_params", typed[1], None, g[1])].append(f"{case_id}::{name}")
            for side, claim in (("polyglot", column.get("nullability")), ("sqlquack", quack_null)):
                if claim in (None, "unknown"):
                    nulls[f"{side}_unknown"] += 1
                elif (claim == "nullable") == g_null:
                    nulls[f"{side}_right"] += 1
                else:
                    nulls[f"{side}_wrong:{claim}"] += 1
    print("## classes", dict(classes.most_common()))
    print("## params", dict(params))
    print("## nullability", dict(sorted(nulls.items())))
    for source, counter in sorted(by_source.items()):
        print(f"## {source}", dict(counter.most_common()))
    print("## patterns (count, class, polyglot, sqlquack, engine)")
    ranked = sorted(patterns.items(), key=lambda item: -len(item[1]))
    for key, items in ranked:
        print(len(items), *key, items[0], sep="\t")
    json.dump(
        {
            "classes": classes,
            "params": params,
            "nullability": nulls,
            "patterns": [{"key": list(k), "count": len(v), "examples": v[:8]} for k, v in ranked],
        },
        open(out_path, "w", encoding="utf-8"),
        indent=1,
    )


main()
