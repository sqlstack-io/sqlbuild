"""The project mutations a step applies before planning."""

from __future__ import annotations

import re
from collections.abc import Callable
from pathlib import Path

import duckdb

from scripts.planner_differential._helpers.capturing.state_tables import database_files
from scripts.planner_differential._helpers.running.commands import run_sqb
from scripts.planner_differential._helpers.steps.model_file import (
    add_header_property,
    join_model_file,
    split_model_file,
)
from scripts.planner_differential._helpers.steps.relations import schema_and_name
from scripts.planner_differential.constants import (
    ADDED_COLUMN_NAME,
    ADDED_MODEL_NAME,
    ADDED_MODEL_PATH,
    EDITED_DESCRIPTION,
    MUTATION_ADD_MODEL,
    MUTATION_CONFIG,
    MUTATION_DECLARE_COLUMN,
    MUTATION_DROP_RELATION,
    MUTATION_EDIT_QUERY,
    MUTATION_MATERIALIZATION,
    MUTATION_RENAME_COLUMN,
    MUTATION_RENAME_MODEL,
    MUTATION_SOURCE_GROWTH,
    MUTATION_TAGS,
    RENAMED_SUFFIX,
    SOURCE_GROWTH_INTERVAL,
)
from scripts.planner_differential.models import (
    CommandResult,
    ModelFacts,
    ModelFile,
    MutationResult,
    StepContext,
)

_TAGS_PATTERN: re.Pattern[str] = re.compile(r"^(\s*tags\s*\[)", re.MULTILINE)
_SCHEMA_CHANGE_KEY: str = "on_schema_change"
_SCHEMA_CHANGE_PATTERN: re.Pattern[str] = re.compile(
    rf"^(\s*{_SCHEMA_CHANGE_KEY}\s+)(\w+)", re.MULTILINE
)
_APPEND_NEW_COLUMNS: str = "append_new_columns"
_SCHEMA_CHANGE_TOGGLE: dict[str, str] = {
    _APPEND_NEW_COLUMNS: "sync_all_columns",
    "sync_all_columns": _APPEND_NEW_COLUMNS,
}
_MATERIALIZED_PATTERN: re.Pattern[str] = re.compile(r"^(\s*materialized\s+)\w+", re.MULTILINE)
_COLUMNS_PATTERN: re.Pattern[str] = re.compile(r"^(\s*columns\s*\()", re.MULTILINE)
_EDIT_ALIAS: str = "planner_differential_edit"
_TAG: str = "planner_differential"
_TIME_TYPES: tuple[str, ...] = ("TIMESTAMP", "DATE")
_COLUMNS_SQL: str = (
    "SELECT column_name, data_type FROM information_schema.columns "
    "WHERE table_schema = ? AND table_name = ? ORDER BY ordinal_position"
)
_SOURCE_COLUMNS_SQL: str = (
    "SELECT c.table_schema, c.column_name, c.data_type FROM information_schema.columns c "
    "JOIN information_schema.tables t "
    "ON t.table_schema = c.table_schema AND t.table_name = c.table_name "
    "WHERE t.table_type = 'BASE TABLE' AND c.table_name = ? "
    "AND (CAST(? AS VARCHAR) IS NULL OR c.table_schema = ?) "
    "ORDER BY c.table_schema, c.ordinal_position"
)
_RELATION_TYPE_SQL: str = (
    "SELECT table_type FROM information_schema.tables WHERE table_schema = ? AND table_name = ?"
)
_VIEW_TYPE: str = "VIEW"


def apply_mutation(*, name: str, context: StepContext, target: ModelFacts) -> MutationResult:
    """Apply the named mutation to the target model or its project."""

    mutations: dict[str, Callable[..., MutationResult]] = {
        MUTATION_EDIT_QUERY: _edit_query,
        MUTATION_TAGS: _tags,
        MUTATION_CONFIG: _config,
        MUTATION_MATERIALIZATION: _materialization,
        MUTATION_DECLARE_COLUMN: _declare_column,
        MUTATION_RENAME_COLUMN: _rename_column,
        MUTATION_ADD_MODEL: _add_model,
        MUTATION_RENAME_MODEL: _rename_model,
        MUTATION_DROP_RELATION: _drop_relation,
        MUTATION_SOURCE_GROWTH: _source_growth,
    }
    return mutations[name](context=context, target=target)


def _edit_query(*, context: StepContext, target: ModelFacts) -> MutationResult:
    return _rewrite_model(
        context=context,
        target=target,
        rewrite=lambda model_file: ModelFile(
            header=model_file.header,
            query=f"WITH {_EDIT_ALIAS} AS (\n{model_file.query}\n)\nSELECT *\nFROM {_EDIT_ALIAS}",
        ),
    )


def _tags(*, context: StepContext, target: ModelFacts) -> MutationResult:
    def rewrite(model_file: ModelFile) -> ModelFile:
        header: str = (
            _TAGS_PATTERN.sub(rf"\g<1>{_TAG}, ", model_file.header, count=1)
            if _TAGS_PATTERN.search(model_file.header)
            else add_header_property(header=model_file.header, line=f"tags [{_TAG}],")
        )
        return ModelFile(header=header, query=model_file.query)

    return _rewrite_model(context=context, target=target, rewrite=rewrite)


def _config(*, context: StepContext, target: ModelFacts) -> MutationResult:
    def rewrite(model_file: ModelFile) -> ModelFile:
        found: re.Match[str] | None = _SCHEMA_CHANGE_PATTERN.search(model_file.header)
        value: str = (
            _SCHEMA_CHANGE_TOGGLE.get(found.group(2), _APPEND_NEW_COLUMNS)
            if found is not None
            else _APPEND_NEW_COLUMNS
        )
        header: str = (
            _SCHEMA_CHANGE_PATTERN.sub(rf"\g<1>{value}", model_file.header, count=1)
            if found is not None
            else add_header_property(
                header=model_file.header, line=f"{_SCHEMA_CHANGE_KEY} {value},"
            )
        )
        return ModelFile(header=header, query=model_file.query)

    return _rewrite_model(context=context, target=target, rewrite=rewrite)


def _materialization(*, context: StepContext, target: ModelFacts) -> MutationResult:
    def rewrite(model_file: ModelFile) -> ModelFile:
        header: str = (
            _MATERIALIZED_PATTERN.sub(r"\g<1>view", model_file.header, count=1)
            if _MATERIALIZED_PATTERN.search(model_file.header)
            else add_header_property(header=model_file.header, line="materialized view,")
        )
        return ModelFile(header=header, query=model_file.query)

    return _rewrite_model(context=context, target=target, rewrite=rewrite)


def _declare_column(*, context: StepContext, target: ModelFacts) -> MutationResult:
    declaration: str = f'{ADDED_COLUMN_NAME} (description "{EDITED_DESCRIPTION}"),'
    columns: list[str] = [
        f'{_EDIT_ALIAS}."{column}"'
        for column, _ in _relation_columns(context=context, target=target)
    ]
    if not columns:
        return MutationResult(applied=False, note=f"no built relation for {target.name}")

    def rewrite(model_file: ModelFile) -> ModelFile:
        header: str = (
            _COLUMNS_PATTERN.sub(rf"\g<1>\n    {declaration}", model_file.header, count=1)
            if _COLUMNS_PATTERN.search(model_file.header)
            else add_header_property(header=model_file.header, line=f"columns ({declaration}),")
        )
        return ModelFile(
            header=header,
            query=(
                f"WITH {_EDIT_ALIAS} AS (\n{model_file.query}\n)\n"
                f"SELECT {', '.join(columns)}, CAST(1 AS INTEGER) AS {ADDED_COLUMN_NAME}\n"
                f"FROM {_EDIT_ALIAS}"
            ),
        )

    return _rewrite_model(context=context, target=target, rewrite=rewrite)


def _rename_column(*, context: StepContext, target: ModelFacts) -> MutationResult:
    path: Path = context.project_dir / target.relative_path
    model_file: ModelFile | None = split_model_file(path.read_text(encoding="utf-8"))
    header: str = "" if model_file is None else model_file.header
    columns: list[str] = [
        column
        for column, _ in _relation_columns(context=context, target=target)
        if column != target.cursor_column
        and column != ADDED_COLUMN_NAME
        and re.search(rf"\b{re.escape(column)}\b", header) is None
    ]
    if not columns:
        return MutationResult(applied=False, note=f"no renameable column in {target.name}")
    column: str = columns[-1]
    return _sqb_mutation(
        context=context,
        arguments=("rename", f"{target.name}.{column}", f"{column}{RENAMED_SUFFIX}"),
    )


def _add_model(*, context: StepContext, target: ModelFacts) -> MutationResult:
    path: Path = context.project_dir / ADDED_MODEL_PATH
    _ = path.write_text(
        f'MODEL (\n  materialized table,\n  description "{EDITED_DESCRIPTION}",\n);\n\n'
        f'SELECT *\nFROM __ref("{target.name}")\n',
        encoding="utf-8",
        newline="\n",
    )
    return MutationResult(applied=True, note=f"added {ADDED_MODEL_NAME} reading {target.name}")


def _rename_model(*, context: StepContext, target: ModelFacts) -> MutationResult:
    return _sqb_mutation(
        context=context, arguments=("rename", target.name, f"{target.name}{RENAMED_SUFFIX}")
    )


def _drop_relation(*, context: StepContext, target: ModelFacts) -> MutationResult:
    schema, name = schema_and_name(target.qualified_name)
    for database in database_files(context.project_dir):
        with duckdb.connect(str(database)) as connection:
            found: tuple[object, ...] | None = connection.execute(
                _RELATION_TYPE_SQL, [schema, name]
            ).fetchone()
            if found is None:
                continue
            kind: str = "VIEW" if found[0] == _VIEW_TYPE else "TABLE"
            _ = connection.execute(f'DROP {kind} "{schema}"."{name}"')
            return MutationResult(applied=True, note=f"dropped {kind.lower()} {schema}.{name}")
    return MutationResult(applied=False, note=f"no relation {target.qualified_name}")


def _source_growth(*, context: StepContext, target: ModelFacts) -> MutationResult:
    _ = target
    grown: list[str] = []
    for source in context.source_tables:
        schema: str | None = source.rpartition(".")[0] or None
        name: str = source.rpartition(".")[2]
        for database in database_files(context.project_dir):
            with duckdb.connect(str(database)) as connection:
                columns: list[tuple[str, str, str]] = connection.execute(
                    _SOURCE_COLUMNS_SQL, [name, schema, schema]
                ).fetchall()
                time_column: tuple[str, str, str] | None = next(
                    (column for column in columns if column[2].startswith(_TIME_TYPES)), None
                )
                if time_column is None:
                    continue
                table_schema, column, column_type = time_column
                relation: str = f'"{table_schema}"."{name}"'
                _ = connection.execute(
                    f'INSERT INTO {relation} SELECT * REPLACE (CAST("{column}" + '
                    f'{SOURCE_GROWTH_INTERVAL} AS {column_type}) AS "{column}") '
                    f'FROM {relation} WHERE "{column}" = (SELECT max("{column}") FROM {relation})'
                )
                grown.append(f"{table_schema}.{name}.{column}")
    if not grown:
        return MutationResult(applied=False, note="no source table with a time column")
    return MutationResult(applied=True, note=f"grew {', '.join(grown)}")


def _rewrite_model(
    *, context: StepContext, target: ModelFacts, rewrite: Callable[[ModelFile], ModelFile]
) -> MutationResult:
    path: Path = context.project_dir / target.relative_path
    model_file: ModelFile | None = split_model_file(path.read_text(encoding="utf-8"))
    if model_file is None:
        return MutationResult(applied=False, note=f"cannot edit {target.relative_path}")
    _ = path.write_text(join_model_file(rewrite(model_file)), encoding="utf-8", newline="\n")
    return MutationResult(applied=True, note=f"edited {target.relative_path}")


def _sqb_mutation(*, context: StepContext, arguments: tuple[str, ...]) -> MutationResult:
    result: CommandResult = run_sqb(
        arguments=arguments,
        project_dir=context.project_dir,
        environment=context.environment,
        python=context.python,
    )
    return MutationResult(
        applied=result.exit_code == 0,
        note=f"sqb {' '.join(arguments)} exited {result.exit_code}",
    )


def _relation_columns(*, context: StepContext, target: ModelFacts) -> list[tuple[str, str]]:
    schema, name = schema_and_name(target.qualified_name)
    for database in database_files(context.project_dir):
        with duckdb.connect(str(database), read_only=True) as connection:
            columns: list[tuple[str, str]] = connection.execute(
                _COLUMNS_SQL, [schema, name]
            ).fetchall()
        if columns:
            return columns
    return []
