from __future__ import annotations

from dataclasses import dataclass

from sqlbuild._native import DbtInteropSkipReason


@dataclass(frozen=True)
class RealDbtRunnerTestCase:
    description: str
    select: tuple[str, ...]
    exclude: tuple[str, ...]
    resource_types: tuple[str, ...]
    expected_unique_ids: tuple[str, ...]


@dataclass(frozen=True)
class RealDbtManifestCompileTestCase:
    description: str
    sqlbuild_model_sql: str
    expected_compiled_sql: str


@dataclass(frozen=True)
class RealDbtCombinedGraphTestCase:
    description: str
    sqlbuild_model_sql_by_name: dict[str, str]
    expected_downstream_from: str
    expected_downstream_keys: tuple[str, ...]


@dataclass(frozen=True)
class RealDbtInteropPlanTestCase:
    description: str
    args: tuple[str, ...]
    sqlbuild_model_contents_by_relative_path: dict[str, str]
    expected_sqlbuild_model_names: tuple[str, ...]
    expected_sqlbuild_command_argvs: tuple[tuple[str, ...], ...]
    expected_dbt_selected_unique_ids: tuple[str, ...]
    expected_dbt_required_unique_ids: tuple[str, ...]
    expected_dbt_required_selector_terms: tuple[str, ...]
    expected_supplemental_dbt_command_argvs: tuple[tuple[str, ...], ...]
    expected_dbt_anchor_terms: tuple[str, ...]
    expected_dbt_anchor_unique_ids_by_term: dict[str, tuple[str, ...]]
    expected_dbt_skip_reason: DbtInteropSkipReason | None
    expected_sqlbuild_skip_reason: DbtInteropSkipReason | None
    expected_path_translations: tuple[tuple[str, str], ...] = ()


@dataclass(frozen=True)
class DbtMacroBuildTestCase:
    description: str
    files: dict[str, str]
    query: str
    expected_rows: tuple[tuple[object, ...], ...]
