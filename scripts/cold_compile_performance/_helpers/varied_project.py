"""Generate diverse order-query shapes over a shared, bounded dependency graph."""

import re
from pathlib import Path

from scripts.cold_compile_performance._helpers.dense_project import (
    dense_model_name,
    write_dense_compile_project,
)
from scripts.cold_compile_performance.constants import (
    DENSE_ALL_RULE_SELECT,
    VARIED_ARRAY_VARIANT,
    VARIED_CASE_SHARE,
    VARIED_GROUP_SHARE,
    VARIED_WINDOW_SHARE,
    VARIED_WINDOW_VARIANTS,
)

_QUANTITY_EXPRESSION: str = "COALESCE(b.amount, 0) + CAST(b.id AS DOUBLE)"
_QUANTITY_VARIANTS: tuple[str, ...] = (
    _QUANTITY_EXPRESSION,
    "CASE WHEN b.id IS NOT NULL THEN b.amount + b.id END",
    "ABS(b.amount) + ABS(b.id)",
    "SUM(b.amount) OVER (PARTITION BY b.id) + b.id",
    "MAX(b.amount) OVER (PARTITION BY b.id) + b.id",
    "COALESCE(NULLIF(b.amount, 0), b.id) + b.id",
    "[b.amount, b.amount][1] + b.id",
    "GREATEST(b.amount, b.id) + LEAST(b.amount, b.id)",
)
_IMPORT: re.Pattern[str] = re.compile(r'__(?:ref|source)\("([^"\n]+)"\)')


def write_varied_compile_project(*, project_dir: Path, model_count: int) -> None:
    """Write independent neutral SQL with diverse expressions and shared ancestors."""

    write_dense_compile_project(
        project_dir=project_dir, model_count=model_count, rule_select=DENSE_ALL_RULE_SELECT
    )
    for index, model in enumerate(sorted((project_dir / "sql" / "models").rglob("*.sql"))):
        contents: str = model.read_text(encoding="utf-8")
        replacements: dict[str, str] = _dependency_replacements(
            index=index, contents=contents, model_count=model_count
        )
        for previous, current in replacements.items():
            contents = contents.replace(previous, current)
        contents = _vary_expressions(index=index, contents=contents)
        contents = _vary_query_structure(index=index, contents=contents)
        model.write_text(contents, encoding="utf-8")
        test_name: str = (
            f"test_{dense_model_name(index).replace('__', '_')}__preserves_order_id.sql"
        )
        test: Path = next((project_dir / "tests").rglob(test_name))
        test_sql: str = test.read_text(encoding="utf-8")
        for previous, current in replacements.items():
            test_sql = test_sql.replace(_mock_name(previous), _mock_name(current))
        test.write_text(test_sql, encoding="utf-8")


def _vary_expressions(*, index: int, contents: str) -> str:
    fragments: list[str] = contents.split(_QUANTITY_EXPRESSION)
    output: list[str] = [fragments[0]]
    for position, fragment in enumerate(fragments[1:]):
        selector: int = ((index + 1) * 2654435761) ^ ((position + 1) * 2246822519)
        variant: int = (selector >> ((position + 1) % 19)) % len(_QUANTITY_VARIANTS)
        if variant == VARIED_ARRAY_VARIANT and index % 5 != 0:
            variant = 0
        if variant in VARIED_WINDOW_VARIANTS and index % 20 >= VARIED_WINDOW_SHARE:
            variant = 2
        if variant == 1 and index % 10 >= VARIED_CASE_SHARE:
            variant = 5
        output.extend((_QUANTITY_VARIANTS[variant], fragment))
    return "".join(output)


def _vary_query_structure(*, index: int, contents: str) -> str:
    if index % 4 != 1:
        contents = re.sub(
            r"combined AS \(\n(SELECT [^\n]+ FROM projected)\n.*?\n\)",
            r"combined AS (\n\1\n)",
            contents,
            flags=re.DOTALL,
        )
    prefix, marker, tail = contents.partition("projected AS (")
    parts: tuple[str, str, str] = tail.partition("),\ncombined AS (")
    projected: str = parts[0]
    separator: str = parts[1]
    rest: str = parts[2]
    if index % 7 == 0:
        projected = projected.replace("b.amount", "quantities.quantity", 1)
        projected += (
            "\nINNER JOIN LATERAL (SELECT b.amount AS quantity) AS quantities"
            " ON quantities.quantity = b.amount"
        )
    if index % 10 < VARIED_GROUP_SHARE:
        keys: list[str] = ["b.id", "b.amount"]
        if index % 7 == 0:
            keys.append("quantities.quantity")
        keys.extend(re.findall(r"COALESCE\((l\d+\.amount), 0\)", projected))
        projected += "\nGROUP BY " + ", ".join(dict.fromkeys(keys))
    return prefix + marker + projected + separator + rest


def _dependency_replacements(*, index: int, contents: str, model_count: int) -> dict[str, str]:
    cohort_start: int = index // 1000 * 1000
    local_index: int = index - cohort_start
    cohort_size: int = min(1000, model_count - cohort_start)
    roots: int = max(1, cohort_size * 14 // 100)
    width: int = max(1, cohort_size * 17 // 1000)
    source_count: int = max(1, cohort_size * 24 // 100)
    imports: list[str] = list(dict.fromkeys(match.group(0) for match in _IMPORT.finditer(contents)))
    replacements: dict[str, str] = {}
    used: set[int] = set()
    for position, imported in enumerate(imports):
        if local_index < roots:
            replacements[imported] = (
                f'__source("orders_{cohort_start * 24 // 100 + local_index % source_count:05d}")'
                if position == 0
                else imported
            )
        elif position == 0:
            level: int = (local_index - roots) // width
            start: int = roots + (level - 1) * width if level else 0
            count: int = width if level else roots
            slot: int = local_index - local_index % 3 if local_index % 3 == 1 else local_index
            parent: int = cohort_start + start + (slot * 7 + 3) % count
            used.add(parent)
            replacements[imported] = f'__ref("{dense_model_name(parent)}")'
        elif position % 2 == 0:
            parent = cohort_start + (local_index * 31 + position * 13) % roots
            if parent not in used:
                used.add(parent)
                replacements[imported] = f'__ref("{dense_model_name(parent)}")'
    return replacements


def _mock_name(reference: str) -> str:
    return (
        reference.replace('__ref("', "__ref__")
        .replace('__source("', "__source__")
        .removesuffix('")')
    )
