"""DuckShop house rule that keeps the edge-case area selectable on its own."""

import re

from sqlbuild.rules import Finding, Model, RuleContext, rule

EDGE_FOLDER = "edge_cases"
EDGE_TAG = "edge_case"
_TAGS_KEY = re.compile(r"tags\s*\[([^\]]*)\]")
_QUOTES = ('"""', '"', "'")


def _string_end(sql: str, start: int) -> int:
    """The index after the string literal opening at `start`."""

    quote = next(q for q in _QUOTES if sql.startswith(q, start))
    index = start + len(quote)
    while index < len(sql):
        if len(quote) == 1 and sql[index] == "\\":
            index += 2
        elif sql.startswith(quote, index):
            return index + len(quote)
        else:
            index += 1
    return len(sql)


def _header_tags(sql: str) -> tuple[set[str], int]:
    """The tags the MODEL header declares at its top level, and their line."""

    depth = 0
    index = sql.find("(")
    while 0 <= index < len(sql):
        char = sql[index]
        if char in "\"'":
            index = _string_end(sql, index)
            continue
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                break
        elif depth == 1 and sql.startswith("tags", index) and not sql[index - 1].isalnum():
            match = _TAGS_KEY.match(sql, index)
            if match is not None:
                tags = {t.strip().strip("\"'") for t in match.group(1).split(",") if t.strip()}
                return tags, sql[:index].count("\n") + 1
        index += 1
    return set(), 1


@rule(
    code="XSQBRDUCK002",
    message="Models in an edge_cases folder, and only those, must declare the edge_case tag",
    remediation=(
        "Add `tags [edge_case]` to the header of every model under sql/models/<layer>/edge_cases/, "
        "and move a model that declares the tag into that folder, so benchmark and core runs can "
        "exclude the edge-case area with --exclude tag:edge_case."
    ),
)
def edge_area_is_tagged(*, model: Model, ctx: RuleContext) -> list[Finding]:
    parts = ctx.project.tree.relative_parts(path=model.path, under="sql/models")
    in_area = EDGE_FOLDER in parts
    tags, line = _header_tags(ctx.sql.for_model(model).authored.source)
    if in_area == (EDGE_TAG in tags):
        return []
    return [ctx.finding(subject=model, line=line)]
