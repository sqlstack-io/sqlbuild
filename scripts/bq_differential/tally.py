"""Tally the BigQuery parse differential by source, verdict and gap construct, and against GoogleSQL.

Usage: tally.py BODIES_JSONL RESULTS_JSONL DETAIL_OUT_JSON ORACLE_CACHE_JSON.
"""
import collections, json, sys
bodies = {json.loads(l)["id"]: json.loads(l) for l in open(sys.argv[1])}
rows = [json.loads(l) for l in open(sys.argv[2])]
oracle = json.load(open(sys.argv[4]))
vs = collections.defaultdict(collections.Counter)
table = collections.defaultdict(collections.Counter)
gaps = collections.Counter(); gap_hits = collections.Counter()
detail = collections.defaultdict(list)
for r in rows:
    b = bodies[r["id"]]
    key = b["source"]
    if "normalize_error" in r:
        cell = "normalize_error"
    else:
        p = r["polyglot"]; q = r["sqlquack"] == "ok"
        cell = {(True, True): "both", (True, False): "polyglot_only", (False, True): "sqlquack_only", (False, False): "neither"}[(p, q)]
        if q and r["gaps"]:
            cell += "+gapped"
            for g in {g["construct"] for g in r["gaps"]}:
                gaps[g] += 1; gap_hits[g] += b["count"]
    table[key][cell] += 1; table["TOTAL"][cell] += 1
    if "sql" in r:
        g = oracle[r["sql"]] == "accept"; q = r["sqlquack"] == "ok"
        vcell = {(True, True): "agree_accept", (False, False): "agree_reject", (True, False): "quack_false_reject", (False, True): "quack_false_accept"}[(g, q)]
        pcell = {(True, True): "agree_accept", (False, False): "agree_reject", (True, False): "polyglot_false_reject", (False, True): "polyglot_false_accept"}[(g, r["polyglot"])]
        vs[key][vcell] += 1; vs["TOTAL"][vcell] += 1; vs[key][pcell] += 0
        vs[key]["P:" + pcell] += 1; vs["TOTAL"]["P:" + pcell] += 1
        detail[vcell].append(r["id"])
    detail[cell].append(r["id"])
cols = ["both", "both+gapped", "polyglot_only", "sqlquack_only", "sqlquack_only+gapped", "neither", "neither+gapped", "normalize_error"]
print(f"{'source':10}", *[f"{c:>15}" for c in cols])
for k in sorted(table, key=lambda k: (k == "TOTAL", k)):
    print(f"{k:10}", *[f"{table[k][c]:>15}" for c in cols])
print("gaps (distinct statements / weighted hits):")
for g, n in gaps.most_common():
    print(f"  {g:30} {n:4} {gap_hits[g]:5}")
json.dump(detail, open(sys.argv[3], "w"), indent=1)

cols2 = ["agree_accept", "agree_reject", "quack_false_reject", "quack_false_accept", "P:polyglot_false_reject", "P:polyglot_false_accept"]
print("against GoogleSQL's parser:")
print(f"{'source':10}", *[f"{c:>24}" for c in cols2])
for k in sorted(vs, key=lambda k: (k == "TOTAL", k)):
    print(f"{k:10}", *[f"{vs[k][c]:>24}" for c in cols2])
json.dump(detail, open(sys.argv[3], "w"), indent=1)
