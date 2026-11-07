"""Judge each compiled output's type three ways: Polyglot, SQLQuack and GoogleSQL's analyzer.

Usage: semantic_types.py CASES_JSONL RESULTS_JSONL ORACLE_JSONL OUT_JSON

Types are compared by their BigQuery base type (parameters dropped; DOUBLE is FLOAT64). Each
output lands in one class: `agree` (all three), `polyglot_wrong` (SQLQuack matches the analyzer),
`sqlquack_wrong` (Polyglot matches the analyzer), `both_wrong`, `sqlquack_untyped`,
`polyglot_untyped_quack_right`, `polyglot_untyped_quack_wrong`, `both_untyped`. Outputs of models
the analyzer could not type are counted by the analyzer's reason.
"""

import collections
import json
import re
import sys

ALIASES = [
    (r"^(INTEGER|INT|BIGINT|SMALLINT|TINYINT|BYTEINT|INT64|HUGEINT)$", "INT64"),
    (r"^(DOUBLE|DOUBLE PRECISION|FLOAT|FLOAT64|REAL|FLOAT8)$", "FLOAT64"),
    (r"^(BOOLEAN|BOOL)$", "BOOL"),
    (r"^(VARCHAR|TEXT|CHAR|STRING|CHARACTER VARYING|NVARCHAR)$", "STRING"),
    (r"^(DECIMAL|NUMERIC)$", "NUMERIC"),
    (r"^(BIGDECIMAL|BIGNUMERIC)$", "BIGNUMERIC"),
    (r"^(TIMESTAMPTZ|TIMESTAMP WITH TIME ZONE|TIMESTAMP_TZ|TIMESTAMP_LTZ)$", "TIMESTAMP"),
    (r"^(TIMESTAMP_NTZ|TIMESTAMP WITHOUT TIME ZONE)$", "DATETIME"),
]


def base(spelling):
    if not spelling:
        return None
    text = " ".join(spelling.upper().split())
    if text in ("UNKNOWN", "NULL"):
        return None
    if not text.startswith(("ARRAY", "STRUCT")):
        text = re.sub(r"\(.*\)$", "", text).strip()
    for pattern, replacement in ALIASES:
        if re.match(pattern, text):
            return replacement
    return text.replace("DOUBLE", "FLOAT64")


cases = {case["id"]: case for case in map(json.loads, open(sys.argv[1], encoding="utf-8"))}
results = {r["id"]: r for r in map(json.loads, open(sys.argv[2], encoding="utf-8"))}
oracle = {r["id"]: r for r in map(json.loads, open(sys.argv[3], encoding="utf-8"))}
classes = collections.Counter()
by_source = collections.defaultdict(collections.Counter)
patterns = collections.defaultdict(list)
for case_id, result in results.items():
    if "#" in case_id or result["parse"] != "ok":
        continue
    polyglot = cases[case_id]["polyglot"]["columns"]
    if polyglot is None:
        continue
    source = cases[case_id]["source"]
    verdict = oracle.get(case_id, {"status": "missing"})
    poly = {c["name"].lower(): c["data_type"] for c in polyglot}
    quack = {name.lower(): data_type for name, data_type, _ in result["sqlquack_outputs"]}
    if verdict["status"] != "ok":
        reason = verdict.get("detail") or ""
        match = re.search(r"INVALID_ARGUMENT: ([^\[\n]*)", reason)
        label = "analyzer_" + verdict["status"] + ": " + re.sub(r"\d+", "N", (match.group(1) if match else reason.split(" ")[0]).strip())[:60]
        classes[label] += len(quack)
        by_source[source][label] += len(quack)
        continue
    google = {name.lower(): data_type for name, data_type in verdict["outputs"]}
    for name, quack_type in quack.items():
        p, q, g = base(poly.get(name)), base(quack_type), base(google.get(name))
        if g is None:
            label = "analyzer_unnamed"
        elif p == g and q == g:
            label = "agree"
        elif q is None and p is None:
            label = "both_untyped"
        elif q is None:
            label = "sqlquack_untyped" if p == g else "sqlquack_untyped_polyglot_wrong"
        elif p is None:
            label = "polyglot_untyped_quack_right" if q == g else "polyglot_untyped_quack_wrong"
        elif q == g:
            label = "polyglot_wrong"
        elif p == g:
            label = "sqlquack_wrong"
        else:
            label = "both_wrong"
        classes[label] += 1
        by_source[source][label] += 1
        if label != "agree":
            patterns[(label, p, q, g)].append(f"{case_id}::{name}")
print("## classes", dict(classes.most_common()))
for source, counter in sorted(by_source.items()):
    print(f"## {source}", dict(counter.most_common()))
print("## patterns (count, class, polyglot, sqlquack, analyzer)")
ranked = sorted(patterns.items(), key=lambda item: -len(item[1]))
for key, items in ranked:
    print(len(items), *key, items[0], sep="\t")
json.dump(
    {"classes": classes, "patterns": [{"key": list(k), "count": len(v), "examples": v[:8]} for k, v in ranked]},
    open(sys.argv[4], "w", encoding="utf-8"),
    indent=1,
)
