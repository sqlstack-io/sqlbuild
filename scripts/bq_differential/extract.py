"""Collect every BigQuery SQL body SQLBuild generates or tests into one JSON-lines file.

Usage: extract.py OUT --capture LABEL=WORK_DIR ... [--sqlclean FORMAT_CORPUS_JSON]
[--rust-fixtures CRATES_DIR] [--adapter ADAPTER_JSONL].

Each line: {"id", "source", "area", "sql", "normalize", "count"}; `count` is how many times the
same (area, text) occurred, so gaps can be ranked by how often SQLBuild hits them.
"""

import collections
import glob
import json
import re
import sys
from pathlib import Path

import argparse

parser = argparse.ArgumentParser()
parser.add_argument("out", type=Path)
parser.add_argument("--capture", action="append", default=[], help="LABEL=compiler differential work dir")
parser.add_argument("--sqlclean", type=Path)
parser.add_argument("--rust-fixtures", type=Path)
parser.add_argument("--adapter", type=Path)
options = parser.parse_args()
OUT = options.out
CAPTURES = dict(item.split("=", 1) for item in options.capture)
SQLCLEAN = options.sqlclean
RUST_FIXTURES = options.rust_fixtures
ADAPTER = options.adapter
STATEMENT = re.compile(r"^\s*(\(\s*)*(SELECT|WITH|CREATE|INSERT|MERGE|UPDATE|DELETE|DROP|ALTER|TRUNCATE|DECLARE|BEGIN|FROM)\b", re.I)


def load(path):
    document = json.load(open(path))
    shared = document["__shared_nodes__"]

    def resolve(value):
        if isinstance(value, dict):
            if set(value) == {"__shared__"}:
                return resolve(shared[value["__shared__"]])
            return {key: resolve(item) for key, item in value.items()}
        if isinstance(value, list):
            return [resolve(item) for item in value]
        return value

    return resolve(document["__capture__"])


def walk(value):
    if isinstance(value, dict):
        yield value
        for item in value.values():
            yield from walk(item)
    elif isinstance(value, list):
        for item in value:
            yield from walk(item)


def capture_bodies(source, root):
    for path in sorted(glob.glob(f"{root}/*/captures/0-compile/003-compiled_project.json")):
        project = load(path)
        if project.get("sql_analysis_dialect") not in (None, "bigquery"):
            continue
        name = Path(path).parents[2].name
        for model in project["models"]:
            if model.get("query_sql"):
                values = ((model.get("config") or {}).get("values") or {})
                placeholders = {str(k): str(v) for k, v in (values.get("placeholders") or {}).items()}
                yield f"{source}/{name}/model/{model['name']}", "model", model["query_sql"], placeholders or True
        for audit in project["audits"]:
            for field in ("sql_body", "measure_sql", "evidence_sql"):
                if audit.get(field):
                    yield f"{source}/{name}/audit/{audit['name']}/{field}", "audit", audit[field], True
        for test in project["sql_tests"]:
            if test.get("sql_body"):
                yield f"{source}/{name}/test/{test['name']}", "test", test["sql_body"], True
        for function in project["functions"]:
            language = (function.get("language") or {}).get("__enum__", "")
            body = function.get("body_sql")
            if not body or not language.endswith(".SQL"):
                continue
            text = body if STATEMENT.match(body) else f"SELECT ({body})"
            yield f"{source}/{name}/udf/{function['name']}", "udf", text, True
        seen = set()
        for node in walk(project):
            if node.get("__type__", "").endswith(":SqlHookEntry") and node.get("statement"):
                if node["statement"] in seen:
                    continue
                seen.add(node["statement"])
                yield f"{source}/{name}/hook/{node['name']}", "hook", node["statement"], False


def sqlclean_bodies():
    if SQLCLEAN is None:
        return
    for index, case in enumerate(json.load(open(SQLCLEAN))):
        if case.get("dialect") != "bigquery":
            continue
        yield f"sqlclean/{index}", "sqlclean", case["sql"], False
        if case.get("formatted") and case["formatted"] != case["sql"]:
            yield f"sqlclean/{index}/formatted", "sqlclean", case["formatted"], False


RUST_STRING = re.compile(r'r(#*)"(.*?)"\1|"((?:[^"\\]|\\.)*)"', re.S)


def rust_unescape(text):
    text = re.sub(r"\\\n\s*", "", text)
    return text.replace('\\"', '"').replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\")


def rust_bodies():
    if RUST_FIXTURES is None:
        return
    for path in sorted(RUST_FIXTURES.rglob("*.rs")):
        if "test" not in str(path):
            continue
        source = path.read_text(encoding="utf-8")
        if "bigquery" not in source.lower():
            continue
        for block_index, block in enumerate(re.split(r"\n(?=\s*(?:#\[test\]|fn |\w+\s*\{|\s*\w+Case\s*\{))", source)):
            if "bigquery" not in block.lower():
                continue
            for match in RUST_STRING.finditer(block):
                text = match.group(2) if match.group(2) is not None else rust_unescape(match.group(3))
                if STATEMENT.match(text) and len(text) > 12:
                    yield f"rust/{path.relative_to(RUST_FIXTURES)}#{block_index}", "rust", text, False


def adapter_bodies():
    if ADAPTER is not None and ADAPTER.exists():
        for line in open(ADAPTER):
            case = json.loads(line)
            yield case["id"], "adapter", case["sql"], False


groups = collections.OrderedDict()
generators = [capture_bodies(source, root) for source, root in CAPTURES.items()]
generators += [sqlclean_bodies(), rust_bodies(), adapter_bodies()]
for generator in generators:
    for case_id, area, sql, normalize in generator:
        source = case_id.split("/", 1)[0]
        key = (source, area, sql, json.dumps(normalize))
        if key in groups:
            groups[key]["count"] += 1
        else:
            groups[key] = {"id": case_id, "source": source, "area": area, "sql": sql, "normalize": bool(normalize), "placeholders": normalize if isinstance(normalize, dict) else {}, "count": 1}
ids = collections.Counter()
for case in groups.values():
    ids[case["id"]] += 1
    if ids[case["id"]] > 1:
        case["id"] = f"{case['id']}~{ids[case['id']]}"
with OUT.open("w") as out:
    for case in groups.values():
        out.write(json.dumps(case, ensure_ascii=False) + "\n")
counts = collections.Counter((case["source"], case["area"]) for case in groups.values())
for key, value in sorted(counts.items()):
    print(*key, value)
