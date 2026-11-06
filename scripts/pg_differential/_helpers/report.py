"""Summarise harness comparisons per measure, with examples of each difference."""

from __future__ import annotations

from collections import Counter, defaultdict
from typing import Any

from scripts.pg_differential.constants import (
    COMPILED_ORIGIN,
    DIALECT_SEPARATOR,
    EXAMPLES_PER_GROUP,
    GAP_SEPARATOR,
    MEASURES,
    MUTATED_ORIGIN,
    MUTATION_SEPARATOR,
    OPAQUE_PARSE,
)


def summarise(*, cases: list[dict[str, Any]], results: list[dict[str, Any]]) -> dict[str, Any]:
    """The summary of every case, and under `sources` of each `<dialect>/<source>`'s cases."""
    source_by_id: dict[str, str] = {
        case["id"]: f"{case['dialect']}{DIALECT_SEPARATOR}{case['source']}" for case in cases
    }
    by_source: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for result in results:
        by_source[source_by_id[result["id"]]].append(result)
    summary: dict[str, Any] = summarise_results(cases=cases, results=results)
    summary["sources"] = {
        source: summarise_results(cases=cases, results=items) for source, items in by_source.items()
    }
    return summary


def summarise_results(
    *, cases: list[dict[str, Any]], results: list[dict[str, Any]]
) -> dict[str, Any]:
    """Counts per parse outcome and per measure verdict, and differences grouped by kind."""
    sql_by_id: dict[str, str] = {case["id"]: case["query_sql"] for case in cases}
    parse: Counter[str] = Counter()
    opaque: Counter[str] = Counter()
    verdicts: dict[str, dict[str, Counter[str]]] = {
        measure: {COMPILED_ORIGIN: Counter(), MUTATED_ORIGIN: Counter()} for measure in MEASURES
    }
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for result in results:
        parse[result["parse"]] += 1
        if result["parse"] == OPAQUE_PARSE:
            opaque.update(result["detail"].split(GAP_SEPARATOR))
        mutated: bool = MUTATION_SEPARATOR in result["id"]
        origin: str = MUTATED_ORIGIN if mutated else COMPILED_ORIGIN
        for measure in MEASURES:
            verdicts[measure][origin][result[measure]] += 1
        for difference in result["differences"]:
            key: str = f"{difference['measure']}:{difference['kind']}"
            example: dict[str, Any] = {**difference, "id": result["id"]}
            groups[key].append({**example, "sql": sql_by_id[result["id"]]})
    ordered: list[tuple[str, list[dict[str, Any]]]] = sorted(
        groups.items(), key=lambda item: -len(item[1])
    )
    return {
        "cases": len(results),
        "parse": dict(parse),
        "opaque_constructs": dict(opaque.most_common()),
        "verdicts": {measure: verdict_counts(verdicts[measure]) for measure in MEASURES},
        "differences": {
            key: {"count": len(items), "examples": items[:EXAMPLES_PER_GROUP]}
            for key, items in ordered
        },
    }


def verdict_counts(origins: dict[str, Counter[str]]) -> dict[str, dict[str, int]]:
    """Plain dictionaries of a measure's verdict counts per case origin."""
    return {origin: dict(counter) for origin, counter in origins.items()}
