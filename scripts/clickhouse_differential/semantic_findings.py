"""Judge each case's binding findings against ClickHouse's verdict on `DESCRIBE (query)`.

Usage: semantic_findings.py RESULTS_JSONL ORACLE_JSONL

Per case (compiled and mutated): whether ClickHouse accepts the query, the error code name of its
refusal, and whether Polyglot and SQLQuack report any compared finding (E2xx). A finding on a query
ClickHouse accepts is a false finding; a refusal of a class SQLBuild checks (names, grouping, types)
that a side does not report is a miss. Each row: count, origin, verdict, sides (P = Polyglot finds,
Q = SQLQuack finds), an example case and its first findings.
"""

import collections
import json
import sys

results = {r["id"]: r for r in map(json.loads, open(sys.argv[1], encoding="utf-8"))}
oracle = {r["id"]: r for r in map(json.loads, open(sys.argv[2], encoding="utf-8"))}
cells = collections.Counter()
examples = {}
for case_id, row in oracle.items():
    if row["status"] == "skipped":
        continue
    result = results[case_id]
    origin = "mutated:" + case_id.split("#", 1)[1] if "#" in case_id else "compiled"
    verdict = "accepts" if row["status"] == "ok" else row["detail"].split(":", 1)[0]
    polyglot = "P" if result["polyglot_findings"] else "-"
    quack = "Q" if result["sqlquack_findings"] else "-"
    key = (origin, verdict, polyglot + quack)
    cells[key] += 1
    examples.setdefault(
        key,
        (case_id, (result["polyglot_findings"] or [""])[0][:90], (result["sqlquack_findings"] or [""])[0][:90],
         "" if row["status"] == "ok" else row["detail"][:120]),
    )
for key, count in sorted(cells.items(), key=lambda item: (item[0][0], item[0][1], -item[1])):
    print(count, *key, *examples[key], sep="\t")
