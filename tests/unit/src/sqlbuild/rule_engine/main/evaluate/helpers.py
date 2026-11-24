"""Policy evaluation test helpers."""

from pathlib import Path

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    CompileSqlReference,
    EnumDeclaration,
    EnumMember,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)

_ENUM_DECLARATION: EnumDeclaration = EnumDeclaration(
    name="status",
    members=(EnumMember(name="ACTIVE", value="active"),),
    scalar_type="VARCHAR",
    relative_path=Path("sql/enums/status.sql"),
    model_name=None,
)


def build_project(
    *,
    name: str,
    relative_path: str,
    sql: str,
    config_values: dict[str, object],
    references: tuple[CompileSqlReference, ...] = (),
    authored_sql: str | None = None,
    enum_columns: tuple[str, ...] = (),
) -> CompiledProjectView:
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        deps=(),
        name=name,
        relative_path=Path(relative_path),
        query_sql=sql,
        authored_sql=authored_sql or sql,
        config=CompileModelConfig(values=config_values),
        references=references,
        enum_columns={name: _ENUM_DECLARATION for name in enum_columns},
        destination=CompiledRelationLocation(
            database=None,
            schema=None,
            name=name,
            qualified_name=name,
        ),
    )
    return CompiledProject(
        run_id="test",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=(model,),
    )
