"""Planning workload a large project carries: incremental history, renamed models and exports.

A live plan of a real project reads a cursor bound for every incremental model, plans a
migration for every renamed model and discovers every Python node. The layers above build
few of each, so this module adds them in proportions such projects have: about one model in
twenty kept incrementally, a hundred of those renamed from an older name, and a few hundred
Python exports of published marts, spread over many modules that share helper code.
"""

from __future__ import annotations

from duckshop10k.core import Column, Model, Project

#: Every this-many timestamped intermediate table gets an incrementally kept copy.
INCREMENT_STRIDE = 3
#: One incremental model in this many caps how far ahead its automatic start may sit.
MAX_AHEAD_EVERY = 5
#: One incremental model in this many was renamed from an older name.
RENAMED_EVERY = 5
#: Every this-many published mart gets a Python export asset.
EXPORT_STRIDE = 5
#: Export assets per Python module, so discovery walks many modules.
EXPORTS_PER_MODULE = 6
_TIMESTAMP = "TIMESTAMP_NTZ"
_INCREMENT_FOLDER = "sql/models/intermediate/increments"
_EXPORT_PACKAGE = "python/nodes/exports"
_SHARED_MODULE = "python/nodes/shared/export_feed.py"

SHARED_EXPORT_HELPERS = '''"""What every export asset records about the relation it sent."""

from sqlbuild.assets import AssetContext
from sqlbuild.refs import SqlResourceRef


def feed_metadata(ctx: AssetContext, reference: SqlResourceRef, feed: str) -> dict[str, object]:
    """The relation an export read, its row count and the partner feed it went to."""
    relation = ctx.relation(reference)
    rows = ctx.query(f"SELECT COUNT(*) FROM {relation}").fetchone()[0]
    return {"feed": feed, "relation": str(relation), "rows": rows, "format": feed_format(feed)}


def feed_format(feed: str) -> str:
    """Partner feeds named after a region take CSV; every other feed takes Parquet."""
    return "csv" if feed.endswith(("_eu", "_us", "_apac", "_mea", "_latam")) else "parquet"
'''


def build_workload(project: Project) -> None:
    """Add the incremental, renamed and Python export nodes to the assembled project."""

    increments: list[Model] = _increments(project)
    _rename(increments)
    _exports(project)


def _timestamp_column(model: Model) -> Column | None:
    return next((column for column in model.columns if column.type == _TIMESTAMP), None)


def _is_table(model: Model) -> bool:
    return not any(line.startswith("materialized") for line in model.config)


def _increments(project: Project) -> list[Model]:
    candidates: list[Model] = sorted(
        (
            model
            for model in project.models.values()
            if model.layer == "intermediate"
            and _is_table(model)
            and _timestamp_column(model) is not None
        ),
        key=lambda model: model.name,
    )
    added: list[Model] = []
    for position, upstream in enumerate(candidates[::INCREMENT_STRIDE]):
        cursor: Column = next(column for column in upstream.columns if column.type == _TIMESTAMP)
        config: list[str] = [
            "materialized incremental",
            "incremental_strategy append",
            f"cursor {cursor.name}",
            "cursor_type timestamp",
            "cursor_grain day",
            f"cursor_inputs (\n    {upstream.name} {cursor.name},\n  )",
        ]
        if position % MAX_AHEAD_EVERY == 0:
            config.append("cursor_start_max_ahead 1d")
        columns: list[Column] = [
            Column(column.name, column.type, audits=("not_null",) * (column == cursor))
            for column in upstream.columns
        ]
        select: str = ",\n    ".join(f"s.{column.name}" for column in columns)
        added.append(
            project.add(
                Model(
                    name=f"{upstream.name}_history",
                    folder=f"{_INCREMENT_FOLDER}/{upstream.folder.rsplit('/', 1)[-1]}",
                    layer="intermediate",
                    description=(
                        f"Every row of {upstream.name} kept incrementally by {cursor.name}, so "
                        "earlier days stay queryable after the source rewrites them"
                    ),
                    columns=columns,
                    body=(
                        "WITH source_rows AS (\n  SELECT *\n"
                        f'  FROM __ref("{upstream.name}")\n),\n\n'
                        f"final AS (\n  SELECT\n    {select}\n  FROM source_rows AS s\n)\n\n"
                        "SELECT * FROM final"
                    ),
                    config=config,
                    refs=[upstream.name],
                )
            )
        )
    return added


def _rename(increments: list[Model]) -> None:
    for model in increments[::RENAMED_EVERY]:
        model.config.append(f"migrate_from {model.name.removesuffix('_history')}_archive")


def _exports(project: Project) -> None:
    marts: list[Model] = sorted(
        (model for model in project.models.values() if model.layer == "marts" and _is_table(model)),
        key=lambda model: (model.folder, model.name),
    )
    by_folder: dict[str, list[Model]] = {}
    for model in marts[::EXPORT_STRIDE]:
        by_folder.setdefault(model.folder.rsplit("/", 1)[-1], []).append(model)
    project.write(_SHARED_MODULE, SHARED_EXPORT_HELPERS)
    for folder, models in sorted(by_folder.items()):
        for start in range(0, len(models), EXPORTS_PER_MODULE):
            chunk: list[Model] = models[start : start + EXPORTS_PER_MODULE]
            module: str = f"{folder}_{start // EXPORTS_PER_MODULE + 1:02d}"
            project.write(f"{_EXPORT_PACKAGE}/{module}.py", _export_module(folder, chunk))


def _export_module(folder: str, models: list[Model]) -> str:
    assets: list[str] = [
        f'@asset(depends_on=model("{model.name}"))\n'
        f"def export_{model.name}(ctx: AssetContext):\n"
        f'    """Send {model.name} to the {folder} partner feed."""\n'
        f'    metadata = feed_metadata(ctx, model("{model.name}"), "{folder}")\n'
        "    return ctx.result(metadata=metadata, materialized=True)\n"
        for model in models
    ]
    return (
        f'"""Exports of the {folder} marts to partner feeds."""\n\n'
        "from sqlbuild.assets import AssetContext, asset\n"
        "from sqlbuild.refs import model\n\n"
        "from python.nodes.shared.export_feed import feed_metadata\n\n\n" + "\n\n".join(assets)
    )
