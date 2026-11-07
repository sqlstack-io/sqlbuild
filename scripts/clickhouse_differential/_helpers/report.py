"""Summarize the harness comparisons by category, form, kind and gap."""

from __future__ import annotations

from collections import Counter, defaultdict
from typing import Any

from scripts.clickhouse_differential.constants import CATEGORIES, EXAMPLES_PER_GROUP


def unique_bodies(results: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    """Comparisons grouped by the SQL they parsed, so repeated seed bodies count once."""
    groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for result in results:
        groups[result["sql"]].append(result)
    return groups


def short(text: str | None, limit: int = 240) -> str | None:
    """`text` on one line, cut to `limit` characters."""
    if text is None:
        return None
    flat: str = " ".join(text.split())
    return flat if len(flat) <= limit else flat[: limit - 3] + "..."


def example(result: dict[str, Any]) -> dict[str, Any]:
    """The fields a reader needs to reproduce one comparison."""
    return {
        "id": result["id"],
        "sql": short(result["sql"]),
        "polyglot": result["polyglot"],
        "polyglot_message": short(result.get("polyglot_message")),
        "sqlquack": result["sqlquack"],
        "sqlquack_gaps": result["sqlquack_gaps"],
        "sqlquack_message": short(result.get("sqlquack_message")),
    }


def summarize(results: list[dict[str, Any]]) -> dict[str, Any]:
    """Counts over unique bodies, and the bodies that miss the bar."""
    groups: dict[str, list[dict[str, Any]]] = unique_bodies(results)
    firsts: list[dict[str, Any]] = [members[0] for members in groups.values()]
    categories: Counter[str] = Counter(result["category"] for result in firsts)
    by_form: dict[str, Counter[str]] = defaultdict(Counter)
    by_kind: dict[str, Counter[str]] = defaultdict(Counter)
    by_source: dict[str, Counter[str]] = defaultdict(Counter)
    gap_constructs: Counter[str] = Counter()
    gap_forms: Counter[str] = Counter()
    misses: dict[str, list[dict[str, Any]]] = defaultdict(list)
    hits: Counter[str] = Counter()
    for sql, members in groups.items():
        first: dict[str, Any] = members[0]
        for form in sorted(set(first["forms"])) or ["EMPTY"]:
            by_form[form][first["category"]] += 1
        for kind in {member["kind"] for member in members}:
            by_kind[kind][first["category"]] += 1
        for source in {member["source"] for member in members}:
            by_source[source][first["category"]] += 1
        for construct in set(first["sqlquack_gaps"]):
            gap_constructs[construct] += 1
            hits[construct] += len(members)
        if first["sqlquack"] == "gap":
            for form in set(first["forms"]):
                gap_forms[form] += 1
        # The bar: wherever Polyglot parses, SQLQuack parses to a tree with no gap.
        if first["polyglot"] != "error" and first["sqlquack"] != "tree":
            reason: str = "gap" if first["sqlquack"] == "gap" else "sqlquack_error"
            misses[reason].append({**example(first), "copies": len(members), "sql_key": sql})
    return {
        "bodies": len(results),
        "unique_bodies": len(groups),
        "categories": {category: categories.get(category, 0) for category in CATEGORIES},
        "polyglot_opaque": sum(1 for result in firsts if result["polyglot"] == "opaque"),
        "sqlquack_gapped": sum(1 for result in firsts if result["sqlquack"] == "gap"),
        "by_form": {form: dict(counts) for form, counts in sorted(by_form.items())},
        "by_kind": {kind: dict(counts) for kind, counts in sorted(by_kind.items())},
        "by_source": {source: dict(counts) for source, counts in sorted(by_source.items())},
        "gap_constructs": [
            {"construct": construct, "unique_bodies": count, "bodies": hits[construct]}
            for construct, count in gap_constructs.most_common()
        ],
        "gap_forms": dict(gap_forms.most_common()),
        "bar_misses": {
            reason: sorted(items, key=lambda item: -item["copies"])
            for reason, items in sorted(misses.items())
        },
        "neither": [example(result) for result in firsts if result["category"] == "neither"][
            : EXAMPLES_PER_GROUP * 4
        ],
        "sqlquack_only": [
            example(result) for result in firsts if result["category"] == "sqlquack_only"
        ][: EXAMPLES_PER_GROUP * 4],
    }
