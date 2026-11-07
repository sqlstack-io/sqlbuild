"""Summarise the BigQuery semantic differential: verdicts per measure and differences ranked.

Usage: semantic_tally.py CASES_JSONL RESULTS_JSONL SUMMARY_JSON

Prints, per source, each measure's verdicts for compiled models and for mutated variants, then
every difference group (measure, kind and the two sides) ranked by count. The summary file holds
the same with up to five example ids per group.
"""

import collections
import json
import sys

MEASURES = ("lineage", "findings", "types")
VERDICTS = ("agree", "refines", "differ", "not_measured")

cases = {case["id"]: case for case in map(json.loads, open(sys.argv[1], encoding="utf-8"))}
results = [json.loads(line) for line in open(sys.argv[2], encoding="utf-8")]
verdicts = collections.defaultdict(collections.Counter)
parse = collections.defaultdict(collections.Counter)
opaque = collections.Counter()
groups = collections.defaultdict(list)
for result in results:
    source = cases[result["id"]]["source"]
    origin = "mutated" if "#" in result["id"] else "compiled"
    parse[source][result["parse"]] += 1
    if result["parse"] == "opaque":
        opaque.update(result["detail"].split(","))
    for measure in MEASURES:
        verdicts[(source, origin, measure)][result[measure]] += 1
    for difference in result["differences"]:
        if difference["measure"] == "types" and difference["kind"] == "data_type":
            key = (difference["measure"], difference["kind"], difference["polyglot"], difference["sqlquack"])
        elif difference["measure"] == "findings":
            codes = lambda text: " ".join(sorted({item.split(" ", 1)[0] for item in text.split(" | ") if item}))
            key = (difference["measure"], origin, codes(difference["polyglot"]) or "-", codes(difference["sqlquack"]) or "-")
        else:
            key = (difference["measure"], difference["kind"], "", "")
        groups[key].append(result["id"] + ("" if not difference["output"] else f"::{difference['output']}"))

for source in sorted(parse):
    print(f"## {source}: parse {dict(parse[source])}")
    for origin in ("compiled", "mutated"):
        row = []
        for measure in MEASURES:
            counter = verdicts[(source, origin, measure)]
            row.append(f"{measure} " + " / ".join(str(counter[v]) for v in VERDICTS))
        print(f"  {origin}: " + "; ".join(row))
print("## opaque constructs", dict(opaque.most_common()))
print("## differences (count, measure, kind, polyglot, sqlquack)")
ranked = sorted(groups.items(), key=lambda item: -len(item[1]))
for key, items in ranked:
    print(len(items), *key, sep="\t")
json.dump(
    {"groups": [{"key": list(key), "count": len(items), "examples": items[:5]} for key, items in ranked]},
    open(sys.argv[3], "w", encoding="utf-8"),
    indent=1,
)
