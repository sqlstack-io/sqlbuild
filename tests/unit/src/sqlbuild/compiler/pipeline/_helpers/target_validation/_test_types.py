from dataclasses import dataclass

from sqlbuild._native import CompiledRelationLocation, LocalConfig, ProjectConfig
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter


@dataclass(frozen=True)
class ValidateProjectTargetsTestCase:
    description: str
    adapter_name: str
    target: CompiledRelationLocation
    expected_error_fragment: str


@dataclass(frozen=True)
class ValidateManagedWriteSchemaTestCase:
    description: str
    adapter: BaseAdapter
    target_schema: str | None
    effective_connection: dict[str, object]
    expected_error_fragment: str | None
    effective_target_schema: str | None = None


@dataclass(frozen=True)
class ValidateNamedTargetSchemaTestCase:
    description: str
    adapter: BaseAdapter
    project_config: ProjectConfig
    local_config: LocalConfig
    selected_target: str | None
    expected_error_fragment: str | None


@dataclass(frozen=True)
class ValidateManagedLoaderConnectionTestCase:
    description: str
    resolved_connection: dict[str, object]
    expected_schemas: tuple[str, ...]
