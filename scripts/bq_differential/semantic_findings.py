"""Judge each case's binding findings against GoogleSQL's analyzer verdict.

Usage: semantic_findings.py RESULTS_JSONL ORACLE_JSONL

Per case (compiled and mutated): whether the analyzer accepts the query, the class of its first
error, and whether Polyglot and SQLQuack report any compared finding (E2xx). A finding on a query
the analyzer accepts is a false finding; an analyzer error of a class SQLBuild checks (names,
grouping, types) that a side does not report is a miss.
"""

import collections
import json
import re
import sys

CLASSES = [
    (r"Unrecognized name: zq|Table not found|not found inside", "unknown_qualifier"),
    (r"Unrecognized name|Name \S+ not found", "unknown_column"),
    (r"ambiguous", "ambiguous"),
    (r"neither grouped nor aggregated|must be grouped", "grouping"),
    (r"No matching signature|coerce|incompatible types|should return type|Invalid cast", "types"),
    (r"Function not found|Table-valued function not found", "unknown_function"),
    (r"mismatched column count", "set_width"),
    (r"without an OVER", "window"),
]


def error_class(detail):
    for pattern, label in CLASSES:
        if re.search(pattern, detail):
            return label
    match = re.search(r"INVALID_ARGUMENT: ([^\[\n]*)", detail)
    return "other: " + (match.group(1)[:50] if match else detail[:50])


results = {r["id"]: r for r in map(json.loads, open(sys.argv[1], encoding="utf-8"))}
oracle = {r["id"]: r for r in map(json.loads, open(sys.argv[2], encoding="utf-8"))}
cells = collections.Counter()
examples = {}
for case_id, row in oracle.items():
    if row["status"] == "skipped":
        continue
    result = results[case_id]
    origin = "mutated" if "#" in case_id else "compiled"
    verdict = "accepts" if row["status"] == "ok" else error_class(row["detail"])
    polyglot = "P" if result["polyglot_findings"] else "-"
    quack = "Q" if result["sqlquack_findings"] else "-"
    key = (origin, verdict, polyglot + quack)
    cells[key] += 1
    examples.setdefault(key, case_id)
for key, count in sorted(cells.items(), key=lambda item: (item[0][0], item[0][1], -item[1])):
    print(count, *key, examples[key], sep="\t")
