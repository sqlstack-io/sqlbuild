from __future__ import annotations

import pytest

from sqlbuild._native import (
    DbtCombinedGraph,
    DbtInteropArgumentError,
    DbtInteropSelectionResult,
    DbtManifestIndex,
    build_dbt_combined_graph,
    build_dbt_manifest_index,
    resolve_dbt_interop_sqlbuild_selection,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.integrations.dbt._test_types import (
    DbtSelectionErrorTestCase,
    DbtSelectionTestCase,
)
from tests.unit.src.sqlbuild.integrations.dbt.helpers import (
    build_compiled_project_with_model_specs,
    build_manifest_data,
    build_manifest_model_node,
)

_MANIFEST_DATA: dict[str, object] = build_manifest_data(
    nodes=(
        build_manifest_model_node(
            unique_id="model.analytics.stg_orders",
            package_name="analytics",
            name="stg_orders",
            relation_name="analytics.stg_orders",
        ),
        build_manifest_model_node(
            unique_id="model.analytics.int_orders",
            package_name="analytics",
            name="int_orders",
            relation_name="analytics.int_orders",
            depends_on_nodes=("model.analytics.stg_orders",),
        ),
        build_manifest_model_node(
            unique_id="model.analytics.unrelated",
            package_name="analytics",
            name="unrelated",
            relation_name="analytics.unrelated",
        ),
    )
)

_SQLBUILD_SQL: dict[str, str] = {
    "stg_local": "select 1",
    "fact_orders": 'select * from __dbt_ref("int_orders")',
    "mart_orders": 'select * from __ref("fact_orders")',
    "local_only": 'select * from __ref("stg_local")',
}

_SQLBUILD_TAGS: dict[str, tuple[str, ...]] = {
    "fact_orders": ("nightly",),
    "mart_orders": ("nightly",),
}

_SQLBUILD_PATHS: dict[str, str] = {
    "stg_local": "sql/models/staging/stg_local.sql",
    "fact_orders": "sql/models/marts/fact_orders.sql",
    "mart_orders": "sql/models/marts/mart_orders.sql",
    "local_only": "sql/models/local/local_only.sql",
}


@pytest.mark.parametrize(
    "test_case",
    [
        DbtSelectionTestCase(
            description="selects bare SQLBuild model without running upstream dbt",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("fact_orders",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders",),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild leading plus upstream dbt and SQLBuild deps",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+fact_orders",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders",),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild leading plus without inventing dbt work",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+local_only",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("local_only", "stg_local"),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild trailing plus downstream SQLBuild only",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("fact_orders+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild both plus upstream dbt and downstream SQLBuild",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+fact_orders+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild tag directly without running upstream dbt",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("tag:nightly",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild tag upstream expansion",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+tag:nightly",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild tag downstream expansion",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("tag:nightly+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="does not select SQLBuild for dbt leading plus only",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+state:modified",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=(),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="does not select SQLBuild for dbt package selector without downstream",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("package:stripe",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=(),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="selects downstream SQLBuild from dbt trailing plus anchors",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("state:modified+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={"state:modified+": ("model.analytics.int_orders",)},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_dbt_anchor_terms=("state:modified+",),
            expected_dbt_anchor_unique_ids_by_term={
                "state:modified+": ("model.analytics.int_orders",)
            },
        ),
        DbtSelectionTestCase(
            description="selects downstream SQLBuild from dbt package anchors",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("package:stripe+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={"package:stripe+": ("model.analytics.int_orders",)},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_dbt_anchor_terms=("package:stripe+",),
            expected_dbt_anchor_unique_ids_by_term={
                "package:stripe+": ("model.analytics.int_orders",)
            },
        ),
        DbtSelectionTestCase(
            description="selects downstream SQLBuild from dbt source anchors",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("source:stripe.charges+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={
                "source:stripe.charges+": ("model.analytics.int_orders",)
            },
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_dbt_anchor_terms=("source:stripe.charges+",),
            expected_dbt_anchor_unique_ids_by_term={
                "source:stripe.charges+": ("model.analytics.int_orders",)
            },
        ),
        DbtSelectionTestCase(
            description="selects downstream SQLBuild from dbt both-plus anchors",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+state:modified+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={"+state:modified+": ("model.analytics.int_orders",)},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_dbt_anchor_terms=("+state:modified+",),
            expected_dbt_anchor_unique_ids_by_term={
                "+state:modified+": ("model.analytics.int_orders",)
            },
        ),
        DbtSelectionTestCase(
            description="keeps empty dbt anchors from adding SQLBuild work",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("state:modified+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={"state:modified+": ()},
            expected_sqlbuild_model_names=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_anchor_terms=("state:modified+",),
            expected_dbt_anchor_unique_ids_by_term={"state:modified+": ()},
        ),
        DbtSelectionTestCase(
            description="uses only trailing-plus dbt term in mixed union",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("state:modified", "tag:daily+"),
            exclude=(),
            dbt_anchor_unique_ids_by_term={"tag:daily+": ("model.analytics.int_orders",)},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_dbt_anchor_terms=("tag:daily+",),
            expected_dbt_anchor_unique_ids_by_term={"tag:daily+": ("model.analytics.int_orders",)},
        ),
        DbtSelectionTestCase(
            description="uses SQLBuild-owned downstream term but not dbt non-plus anchors",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("state:modified", "fact_orders+"),
            exclude=(),
            dbt_anchor_unique_ids_by_term={"state:modified": ("model.analytics.int_orders",)},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="treats comma expression as one trailing-plus dbt anchor term",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("state:modified,tag:daily+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={
                "state:modified,tag:daily+": ("model.analytics.int_orders",)
            },
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_dbt_anchor_terms=("state:modified,tag:daily+",),
            expected_dbt_anchor_unique_ids_by_term={
                "state:modified,tag:daily+": ("model.analytics.int_orders",)
            },
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild explicit model path directly",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("path:sql/models/marts",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_path_translations=(),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild Windows-style explicit model path directly",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("path:models\\marts",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_path_translations=(("path:models\\marts", "path:sql/models/marts"),),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild explicit model path with upstream expansion",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+path:sql/models/marts",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
            expected_path_translations=(),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild explicit model path with downstream expansion",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("path:sql/models/marts+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
            expected_path_translations=(),
        ),
        DbtSelectionTestCase(
            description="does not select SQLBuild model path without explicit root",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("path:marts",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=(),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="applies SQLBuild exclude to final selected set",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("fact_orders+",),
            exclude=("tag:nightly",),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=(),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="selects SQLBuild to SQLBuild path-between",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("fact_orders~mart_orders",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(),
        ),
        DbtSelectionTestCase(
            description="selects dbt to SQLBuild path-between",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("stg_orders~fact_orders",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders",),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
        ),
        DbtSelectionTestCase(
            description="selects dbt to dbt path-between",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("stg_orders~int_orders",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=(),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
        ),
        DbtSelectionTestCase(
            description="selects dbt to SQLBuild path-between with downstream expansion",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("stg_orders~fact_orders+",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders", "mart_orders"),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
        ),
        DbtSelectionTestCase(
            description="selects dbt to SQLBuild path-between with upstream expansion",
            manifest_data=_MANIFEST_DATA,
            sqlbuild_model_sql_by_name=_SQLBUILD_SQL,
            sqlbuild_model_tags_by_name=_SQLBUILD_TAGS,
            sqlbuild_model_path_by_name=_SQLBUILD_PATHS,
            select=("+int_orders~fact_orders",),
            exclude=(),
            dbt_anchor_unique_ids_by_term={},
            expected_sqlbuild_model_names=("fact_orders",),
            expected_dbt_required_unique_ids=(
                "model.analytics.int_orders",
                "model.analytics.stg_orders",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_interop_selectors_when_resolving_then_returns_expected_sqlbuild_work(
    test_case: DbtSelectionTestCase,
) -> None:
    manifest: DbtManifestIndex = build_dbt_manifest_index(raw_data=test_case.manifest_data)
    project: CompiledProjectView = build_compiled_project_with_model_specs(
        sql_by_model_name=test_case.sqlbuild_model_sql_by_name,
        tags_by_model_name=test_case.sqlbuild_model_tags_by_name,
        path_by_model_name=test_case.sqlbuild_model_path_by_name,
    )
    graph: DbtCombinedGraph = build_dbt_combined_graph(manifest=manifest, project=project)

    result: DbtInteropSelectionResult = resolve_dbt_interop_sqlbuild_selection(
        project=project,
        manifest=manifest,
        graph=graph,
        select=test_case.select,
        exclude=test_case.exclude,
        dbt_anchor_unique_ids_by_term=test_case.dbt_anchor_unique_ids_by_term,
    )

    assert result.sqlbuild_model_names == test_case.expected_sqlbuild_model_names
    assert result.dbt_required_unique_ids == test_case.expected_dbt_required_unique_ids
    assert result.dbt_anchor_terms == test_case.expected_dbt_anchor_terms
    assert result.dbt_anchor_unique_ids_by_term == (
        test_case.expected_dbt_anchor_unique_ids_by_term or {}
    )
    assert result.path_translations == test_case.expected_path_translations


@pytest.mark.parametrize(
    "test_case",
    [
        DbtSelectionErrorTestCase(
            description="ambiguous dbt path endpoint raises clear error",
            manifest_data=build_manifest_data(
                nodes=(
                    build_manifest_model_node(
                        unique_id="model.analytics.stg_orders",
                        package_name="analytics",
                        name="stg_orders",
                        relation_name="analytics.stg_orders",
                    ),
                    build_manifest_model_node(
                        unique_id="model.other.stg_orders",
                        package_name="other",
                        name="stg_orders",
                        relation_name="other.stg_orders",
                    ),
                )
            ),
            sqlbuild_model_sql_by_name={"fact_orders": "select 1"},
            sqlbuild_model_tags_by_name={},
            sqlbuild_model_path_by_name={"fact_orders": "sql/models/marts/fact_orders.sql"},
            select=("stg_orders~fact_orders",),
            expected_error_fragment="endpoint 'stg_orders' is ambiguous across dbt models",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_dbt_interop_path_selector_when_resolving_then_raises_clear_error(
    test_case: DbtSelectionErrorTestCase,
) -> None:
    manifest: DbtManifestIndex = build_dbt_manifest_index(raw_data=test_case.manifest_data)
    project: CompiledProjectView = build_compiled_project_with_model_specs(
        sql_by_model_name=test_case.sqlbuild_model_sql_by_name,
        tags_by_model_name=test_case.sqlbuild_model_tags_by_name,
        path_by_model_name=test_case.sqlbuild_model_path_by_name,
    )
    graph: DbtCombinedGraph = build_dbt_combined_graph(manifest=manifest, project=project)

    with pytest.raises(DbtInteropArgumentError) as exc_info:
        resolve_dbt_interop_sqlbuild_selection(
            project=project,
            manifest=manifest,
            graph=graph,
            select=test_case.select,
        )

    assert test_case.expected_error_fragment in str(exc_info.value)
