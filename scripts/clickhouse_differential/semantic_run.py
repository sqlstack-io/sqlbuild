"""Run the semantic harness with each side typing upstream models itself.

Usage: semantic_run.py HARNESS CASES_JSONL RESULTS_JSONL

SQLBuild hands an undeclared model's inferred columns to the models that read it, so a switch to
SQLQuack would give downstream models SQLQuack's types. The capture's relations carry Polyglot's;
each pass replaces the types of a model relation's undeclared columns with SQLQuack's outputs for that
model from the previous pass, until nothing changes.
"""

import json
import subprocess
import sys

harness, cases_path, results_path = sys.argv[1:4]
cases = [json.loads(line) for line in open(cases_path, encoding="utf-8")]
previous = None
for attempt in range(12):
    text = "".join(json.dumps(case) + "\n" for case in cases)
    output = subprocess.run([harness], input=text, capture_output=True, text=True, check=True).stdout
    results = [json.loads(line) for line in output.splitlines()]
    outputs = {
        r["id"]: {name: data_type for name, data_type, _ in r["sqlquack_outputs"]}
        for r in results
        if "#" not in r["id"] and r["parse"] == "ok"
    }
    changed = 0
    for case in cases:
        project = case["id"].split("#", 1)[0].rsplit("/", 1)[0]
        for relation in case["relations"]:
            if relation["open"] or relation["resource_type"] != "MODEL":
                continue
            upstream = outputs.get(f"{project}/{relation['resource_name']}")
            if upstream is None:
                continue
            for column in relation["columns"]:
                if column["declared"]:
                    continue
                new = upstream.get(column["name"], column["data_type"])
                if new != column["data_type"]:
                    column["data_type"] = new
                    changed += 1
    print(f"pass {attempt}: {changed} upstream column types changed", file=sys.stderr)
    if not changed:
        break
with open(results_path, "w", encoding="utf-8") as out:
    out.write(output)
with open(cases_path.replace(".jsonl", ".chained.jsonl"), "w", encoding="utf-8") as out:
    out.write(text)
