"""Judge each compiled output's type three ways: Polyglot, SQLQuack and ClickHouse's DESCRIBE.

Usage: semantic_types.py CASES_JSONL RESULTS_JSONL ORACLE_JSONL OUT_JSON CLICKHOUSE

Every type spelling either side or the engine gives is canonicalized by ClickHouse itself
(`toTypeName(defaultValueOfTypeName(...))`), so aliases such as INTEGER, VARCHAR(n), DOUBLE or
TIMESTAMP compare as the type ClickHouse makes of them; a spelling ClickHouse refuses is kept as
written and cannot match. `Nullable` and `LowCardinality` are set aside: the type class compares
the inner type with its parameters dropped, `params` counts outputs whose inner types differ only in
parameters, and nullability is judged on its own against whether ClickHouse's type is `Nullable`.

Type classes per output: `agree` (all three), `polyglot_wrong` (SQLQuack matches ClickHouse),
`sqlquack_wrong` (Polyglot matches), `both_wrong`, `sqlquack_untyped`,
`sqlquack_untyped_polyglot_wrong`, `polyglot_untyped_quack_right`, `polyglot_untyped_quack_wrong`,
`both_untyped`. Outputs of cases ClickHouse could not type are counted by its error.
"""

import collections
import json
import re
import subprocess
import sys

WRAPPERS = re.compile(r"^(Nullable|LowCardinality)\((.*)\)$")


def canonical_names(tool, spellings):
    """ClickHouse's own name for each spelling it accepts."""
    spellings = list(spellings)
    literals = ["'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'" for s in spellings]
    queries = [f"SELECT {i}, toTypeName(defaultValueOfTypeName({lit})) FORMAT TSV;" for i, lit in enumerate(literals)]
    completed = subprocess.run(
        [tool, "local", "-n", "--ignore-error"], input="\n".join(queries), capture_output=True, text=True
    )
    names = {}
    for line in completed.stdout.splitlines():
        index, _, name = line.partition("\t")
        names[spellings[int(index)]] = name
    return names


def split(name):
    """`(inner type without parameters, inner type, nullable)` of a canonical ClickHouse name."""
    nullable = False
    while match := WRAPPERS.match(name):
        nullable = nullable or match.group(1) == "Nullable"
        name = match.group(2)
    return re.sub(r"\(.*\)$", "", name), name, nullable


def main():
    cases_path, results_path, oracle_path, out_path, tool = sys.argv[1:6]
    cases = {case["id"]: case for case in map(json.loads, open(cases_path, encoding="utf-8"))}
    results = {r["id"]: r for r in map(json.loads, open(results_path, encoding="utf-8"))}
    oracle = {r["id"]: r for r in map(json.loads, open(oracle_path, encoding="utf-8"))}
    spellings = set()
    for case_id, result in results.items():
        if "#" in case_id or result["parse"] != "ok" or cases[case_id]["polyglot"]["columns"] is None:
            continue
        spellings |= {c["data_type"] for c in cases[case_id]["polyglot"]["columns"] if c["data_type"]}
        spellings |= {t for _, t, _ in result["sqlquack_outputs"] if t}
    names = canonical_names(tool, sorted(spellings))

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
        poly = {c["name"]: c for c in polyglot}
        quack = {name: (data_type, nullability) for name, data_type, nullability in result["sqlquack_outputs"]}
        if verdict["status"] != "ok":
            label = f"engine_{verdict['status']}: " + re.sub(r"\d+", "N", (verdict.get("detail") or "").split(":")[0])[:60]
            classes[label] += len(quack)
            by_source[source][label] += len(quack)
            continue
        engine = {name: canon(data_type) for name, data_type in verdict["outputs"]}
        for name, (quack_type, quack_null) in quack.items():
            column = poly.get(name, {})
            p, q, g = canon(column.get("data_type")), canon(quack_type), engine.get(name)
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
                elif (claim == "nullable") == g[2]:
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
