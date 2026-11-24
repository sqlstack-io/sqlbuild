"""Construct census for the compiled DuckShop project.

Run `sqb compile` in ../project first, then:

    python benchmarks/duckshop/scripts/census.py [--json] [--area core|edge|all]

Constructs are detected with regular expressions over the compiled model SQL, with comments and
string literals removed. A model counts towards a family when the family appears at least once.
The output is aggregate only. By default it counts the core project only: models under an
`edge_cases` folder belong to the edge-case area, which `--area edge` or `--area all` selects.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import statistics
import sys

PROJECT = pathlib.Path(__file__).resolve().parent.parent / "project"
COMPILED = PROJECT / "target" / "compiled" / "sql" / "models"
EDGE_FOLDER = "edge_cases"
AREAS = ("core", "edge", "all")

STRING = re.compile(r"'(?:[^'\\]|\\.|'')*'", re.S)
TOKENS = re.compile(r"('(?:[^'\\]|\\.|'')*')|(/\*.*?\*/)|((?:--|//)[^\n]*)", re.S)

FAMILIES: dict[str, str] = {
    "window": r"\bover\s*\(",
    "qualify": r"\bqualify\b",
    "semi_path": r"[\w\]\)\"]\s*:\s*[\"\w$\[]",
    "flatten": r"\bflatten\s*\(",
    "obj_arr_fn": (
        r"\b(object_\w+|array_\w+|arrayagg|parse_json|try_parse_json|to_variant|to_object"
        r"|to_array|get_path|get_ignore_case|is_object|is_array|typeof|strip_null_value"
        r"|is_null_value)\s*\("
    ),
    "variant_cast": r"::\s*(variant|object|array)\b|\bas\s+(variant|object|array)\s*\)",
    "regex": r"\b(regexp_\w+|rlike|regexp)\b",
    "timezone": (
        r"\b(convert_timezone|timestamp_tz|timestamp_ltz|to_timestamp_tz|to_timestamp_ltz"
        r"|at\s+time\s+zone|sysdate|current_timestamp)\b"
    ),
    "timestamp": (
        r"\b(timestamp(_ntz|_tz|_ltz)?|to_timestamp\w*|try_to_timestamp\w*|dateadd|datediff"
        r"|timestampdiff|timestampadd|date_trunc|time_slice|date_part|extract|last_day)\b"
    ),
    "decimal": (
        r"\b(number|numeric|decimal)\s*\(|::\s*(number|numeric|decimal)\b"
        r"|\bto_(number|decimal|numeric)\s*\(|\bround\s*\("
    ),
    "division": r"[\w\)]\s*/\s*[\w\(]",
    "asof": r"\basof\s+join\b|\bmatch_condition\b",
    "set_op": r"\b(union|intersect|except|minus)\b",
    "pivot": r"\b(pivot|unpivot)\b",
    "generator": r"\bgenerator\s*\(",
    "lateral": r"\blateral\b",
    "hash": r"\bhash\s*\(",
    "ltz": r"\b(timestamp_ltz|to_timestamp_ltz)\b",
    "current_ts": r"\b(current_timestamp|sysdate|getdate|localtimestamp)\b",
    "lambda": r"(\b\w+(\s+\w+)?|\))\s*->",
    "connect_by": r"\bconnect\s+by\b",
    "recursive_cte": r"\bwith\s+recursive\b",
}

# Percent of models, from the DuckShop specification. None means "presence only".
TARGETS: dict[str, float | None] = {
    "semi_any": 60,
    "semi_path": 30,
    "flatten": 15,
    "obj_arr_fn": 40,
    "variant_cast": 45,
    "window": 55,
    "qualify": 45,
    "regex": 30,
    "timestamp": 45,
    "timezone": 15,
    "decimal": 30,
    "division": 15,
    "set_op": 15,
    "lateral": 15,
    "asof": None,
    "hash": None,
    "ltz": None,
    "current_ts": None,
    "lambda": None,
    "pivot": None,
    "generator": None,
    "connect_by": None,
}

FUNCTION_CALL = re.compile(r"\b([a-z_][a-z0-9_]*)\s*\(")
KEYWORD_CONSTRUCTS = {
    "qualify": r"\bqualify\b",
    "asof join": r"\basof\s+join\b",
    "match_condition": r"\bmatch_condition\b",
    "lateral": r"\blateral\b",
    "pivot": r"\bpivot\b",
    "unpivot": r"\bunpivot\b",
    "union all": r"\bunion\s+all\b",
    "union": r"\bunion\b(?!\s+all)",
    "intersect": r"\bintersect\b",
    "except": r"\bexcept\b",
    "minus": r"\bminus\b",
    "connect by": r"\bconnect\s+by\b",
    "within group": r"\bwithin\s+group\b",
    "ignore nulls": r"\bignore\s+nulls\b",
    "rows between": r"\brows\s+between\b",
    "range between": r"\brange\s+between\b",
    "at time zone": r"\bat\s+time\s+zone\b",
    "lambda": r"->",
    "colon path": r"[\w\]\)\"]\s*:\s*[\"\w$\[]",
    "cast ::": r"::",
    "group by all": r"\bgroup\s+by\s+all\b",
    "exclude": r"\bexclude\s*\(",
    "ilike": r"\bilike\b",
    "rlike": r"\brlike\b",
    "full outer join": r"\bfull\s+(outer\s+)?join\b",
    "left join": r"\bleft\s+(outer\s+)?join\b",
    "cross join": r"\bcross\s+join\b",
    "natural join": r"\bnatural\s+join\b",
    "case": r"\bcase\b",
    "distinct": r"\bdistinct\b",
    "having": r"\bhaving\b",
    "interval": r"\binterval\b",
}
NOT_FUNCTIONS = {
    "as",
    "in",
    "and",
    "or",
    "not",
    "over",
    "values",
    "exists",
    "select",
    "from",
    "where",
    "when",
    "then",
    "else",
    "on",
    "using",
    "join",
    "with",
    "table",
    "partition",
    "by",
    "is",
    "filter",
    "lateral",
    "qualify",
    "within",
    "group",
    "pivot",
    "unpivot",
    "for",
    "into",
    "match_condition",
    "any",
    "all",
    "exclude",
    "if",
}


def clean(sql: str) -> str:
    """Remove comments, leaving string literals (which may contain `--` or `//`) intact."""
    return TOKENS.sub(lambda m: m.group(1) or " ", sql)


def analyse(sql: str) -> dict[str, object]:
    text = clean(sql)
    low = STRING.sub("''", text).lower()
    path_source = re.sub(r"::", " CAST_ ", low)
    row: dict[str, object] = {}
    for family, pattern in FAMILIES.items():
        row[family] = bool(re.search(pattern, path_source if family == "semi_path" else low))
    row["semi_any"] = any(row[k] for k in ("semi_path", "flatten", "obj_arr_fn", "variant_cast"))
    row["ctes"] = len(re.findall(r"(?:\bwith\b|,)\s*[\w\"]+\s*(?:\([^)]*\))?\s+as\s*\(", low))
    row["joins"] = len(re.findall(r"\bjoin\b", low))
    row["lines"] = sum(1 for line in text.splitlines() if line.strip())
    functions = {f for f in FUNCTION_CALL.findall(low) if f not in NOT_FUNCTIONS}
    keywords = {k for k, p in KEYWORD_CONSTRUCTS.items() if re.search(p, low)}
    row["constructs"] = sorted(functions | {f"<{k}>" for k in keywords})
    return row


def summarise(rows: list[dict[str, object]]) -> dict[str, object]:
    count = len(rows)
    out: dict[str, object] = {"models": count}
    if not count:
        return out
    for family in [*FAMILIES, "semi_any"]:
        out[f"pct_{family}"] = round(100 * sum(bool(r[family]) for r in rows) / count, 1)
        out[f"n_{family}"] = sum(bool(r[family]) for r in rows)
    for metric in ("ctes", "joins", "lines"):
        values = sorted(int(r[metric]) for r in rows)  # type: ignore[call-overload]
        out[metric] = {
            "mean": round(statistics.mean(values), 1),
            "p50": values[count // 2],
            "p90": values[int(count * 0.9)],
            "max": values[-1],
        }
    return out


def shape(stats: dict[str, float]) -> str:
    return f"mean {stats['mean']} p50 {stats['p50']} p90 {stats['p90']} max {stats['max']}"


def layer_of(path: pathlib.Path) -> str:
    return path.relative_to(COMPILED).parts[0]


def in_area(path: pathlib.Path, area: str) -> bool:
    is_edge = EDGE_FOLDER in path.relative_to(COMPILED).parts
    return area == "all" or is_edge == (area == "edge")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--json", action="store_true", help="print JSON instead of a table")
    parser.add_argument(
        "--area",
        choices=AREAS,
        default="core",
        help="core models only (the default), the edge-case area only, or both",
    )
    args = parser.parse_args()
    files = sorted(path for path in COMPILED.rglob("*.sql") if in_area(path, args.area))
    if not files:
        print(f"no compiled models under {COMPILED}; run `sqb compile` first", file=sys.stderr)
        return 1
    by_layer: dict[str, list[dict[str, object]]] = {}
    for path in files:
        by_layer.setdefault(layer_of(path), []).append(analyse(path.read_text()))
    all_rows = [row for rows in by_layer.values() for row in rows]
    constructs = sorted({c for row in all_rows for c in row["constructs"]})  # type: ignore[attr-defined]
    report = {
        "all": summarise(all_rows),
        "layers": {layer: summarise(rows) for layer, rows in sorted(by_layer.items())},
        "distinct_constructs": len(constructs),
    }
    if args.json:
        report["constructs"] = constructs
        print(json.dumps(report, indent=2))
        return 0
    layers = list(report["layers"])  # type: ignore[arg-type]
    header = f"{'family':<14}{'target':>8}{'all':>8}" + "".join(f"{x[:8]:>10}" for x in layers)
    print(header)
    total = report["all"]
    print(
        f"{'models':<14}{'':>8}{total['models']:>8}"
        + "".join(  # type: ignore[index]
            f"{report['layers'][x]['models']:>10}"
            for x in layers  # type: ignore[index]
        )
    )
    for family, target in TARGETS.items():
        shown = "any" if target is None else f"{target:g}"
        key = f"pct_{family}" if target is not None else f"n_{family}"
        cells = "".join(f"{report['layers'][x][key]:>10}" for x in layers)  # type: ignore[index]
        print(f"{family:<14}{shown:>8}{total[key]:>8}{cells}")  # type: ignore[index]
    for metric in ("ctes", "joins", "lines"):
        print(f"{metric:<14} {shape(total[metric])}")  # type: ignore[index]
        for layer in layers:
            print(f"  {layer:<12} {shape(report['layers'][layer][metric])}")  # type: ignore[index]
    print(f"distinct constructs: {report['distinct_constructs']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
