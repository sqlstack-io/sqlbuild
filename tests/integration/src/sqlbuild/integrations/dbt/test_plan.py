from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import DbtInteropPlan, DbtInteropSkipReason, plan_dbt_interop_from_project
from tests.integration.src.sqlbuild.integrations.dbt._test_types import (
    RealDbtInteropPlanTestCase,
)
from tests.integration.src.sqlbuild.integrations.dbt.helpers import (
    build_sqlbuild_project_with_dbt_config,
    resolve_expected_dbt_argvs,
)

pytestmark: pytest.MarkDecorator = pytest.mark.dbt


@pytest.mark.parametrize(
    "test_case",
    [
        RealDbtInteropPlanTestCase(
            description="plans dbt-only tag selector and skips SQLBuild work",
            args=("--select", "tag:nightly"),
            sqlbuild_model_contents_by_relative_path={
                "local_only.sql": "MODEL (description 'Test model local_only.');\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=(),
            expected_sqlbuild_command_argvs=(),
            expected_dbt_selected_unique_ids=("model.analytics.stg_orders",),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=None,
            expected_sqlbuild_skip_reason=DbtInteropSkipReason.NO_SQLBUILD_WORK,
        ),
        RealDbtInteropPlanTestCase(
            description="plans mixed dbt and SQLBuild tag selector matches",
            args=("--select", "tag:nightly"),
            sqlbuild_model_contents_by_relative_path={
                "tagged_orders.sql": "MODEL (description 'Test model tagged_orders.', tags [nightly]);\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=("tagged_orders",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "tagged_orders"),),
            expected_dbt_selected_unique_ids=("model.analytics.stg_orders",),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=None,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="expands dbt tag trailing plus anchors to downstream SQLBuild models",
            args=("--select", "tag:nightly+"),
            sqlbuild_model_contents_by_relative_path={
                "downstream_orders.sql": (
                    'MODEL (description "Test model downstream_orders.");\n\nselect order_id from __dbt_ref("fact_orders")\n'
                ),
                "mart_orders.sql": (
                    'MODEL (description "Test model mart_orders.");\n\nselect order_id from __ref("downstream_orders")\n'
                ),
            },
            expected_sqlbuild_model_names=("downstream_orders", "mart_orders"),
            expected_sqlbuild_command_argvs=(
                ("sqb", "plan", "--select", "downstream_orders", "mart_orders"),
            ),
            expected_dbt_selected_unique_ids=(
                "model.analytics.fact_orders",
                "model.analytics.stg_orders",
            ),
            expected_dbt_required_unique_ids=("model.analytics.fact_orders",),
            expected_dbt_required_selector_terms=("+fqn:analytics.fact_orders",),
            expected_supplemental_dbt_command_argvs=(
                (
                    "dbt",
                    "ls",
                    "--project-dir",
                    "{dbt_project_dir}",
                    "--profiles-dir",
                    "{dbt_profiles_dir}",
                    "--target-path",
                    "{dbt_target_path}",
                    "--select",
                    "+fqn:analytics.fact_orders",
                ),
            ),
            expected_dbt_anchor_terms=("tag:nightly+",),
            expected_dbt_anchor_unique_ids_by_term={
                "tag:nightly+": (
                    "model.analytics.fact_orders",
                    "model.analytics.stg_orders",
                ),
            },
            expected_dbt_skip_reason=None,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="plans SQLBuild-only tag selector and skips dbt work",
            args=("--select", "tag:sqb_only"),
            sqlbuild_model_contents_by_relative_path={
                "sqb_only.sql": "MODEL (description 'Test model sqb_only.', tags [sqb_only]);\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=("sqb_only",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "sqb_only"),),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=DbtInteropSkipReason.NO_DBT_WORK,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="applies SQLBuild tag excludes after selection",
            args=("--select", "tag:sqb_only", "--exclude", "tag:deprecated"),
            sqlbuild_model_contents_by_relative_path={
                "sqb_only.sql": "MODEL (description 'Test model sqb_only.', tags [sqb_only]);\n\nselect 1 as order_id\n",
                "deprecated_orders.sql": (
                    "MODEL (description 'Test model deprecated_orders.', tags [sqb_only, deprecated]);\n\nselect 2 as order_id\n"
                ),
            },
            expected_sqlbuild_model_names=("sqb_only",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "sqb_only"),),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=DbtInteropSkipReason.NO_DBT_WORK,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="plans SQLBuild-only model and skips dbt command work",
            args=("--select", "local_only"),
            sqlbuild_model_contents_by_relative_path={
                "local_only.sql": "MODEL (description 'Test model local_only.');\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=("local_only",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "local_only"),),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=DbtInteropSkipReason.NO_DBT_WORK,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="plans SQLBuild direct selection with compact dbt upstream boundary",
            args=("--select", "+downstream_orders"),
            sqlbuild_model_contents_by_relative_path={
                "downstream_orders.sql": (
                    'MODEL (description "Test model downstream_orders.");\n\nselect order_id from __dbt_ref("fact_orders")\n'
                ),
                "mart_orders.sql": (
                    'MODEL (description "Test model mart_orders.");\n\nselect order_id from __ref("downstream_orders")\n'
                ),
            },
            expected_sqlbuild_model_names=("downstream_orders",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "downstream_orders"),),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(
                "model.analytics.fact_orders",
                "model.analytics.stg_orders",
            ),
            expected_dbt_required_selector_terms=("+fqn:analytics.fact_orders",),
            expected_supplemental_dbt_command_argvs=(
                (
                    "dbt",
                    "ls",
                    "--project-dir",
                    "{dbt_project_dir}",
                    "--profiles-dir",
                    "{dbt_profiles_dir}",
                    "--target-path",
                    "{dbt_target_path}",
                    "--select",
                    "+fqn:analytics.fact_orders",
                ),
            ),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=None,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="expands real dbt trailing plus anchors to downstream SQLBuild models",
            args=("--select", "fact_orders+"),
            sqlbuild_model_contents_by_relative_path={
                "downstream_orders.sql": (
                    'MODEL (description "Test model downstream_orders.");\n\nselect order_id from __dbt_ref("fact_orders")\n'
                ),
                "mart_orders.sql": (
                    'MODEL (description "Test model mart_orders.");\n\nselect order_id from __ref("downstream_orders")\n'
                ),
            },
            expected_sqlbuild_model_names=("downstream_orders", "mart_orders"),
            expected_sqlbuild_command_argvs=(
                ("sqb", "plan", "--select", "downstream_orders", "mart_orders"),
            ),
            expected_dbt_selected_unique_ids=("model.analytics.fact_orders",),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=("fact_orders+",),
            expected_dbt_anchor_unique_ids_by_term={
                "fact_orders+": ("model.analytics.fact_orders",),
            },
            expected_dbt_skip_reason=None,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="applies SQLBuild excludes after dbt anchor expansion",
            args=("--select", "fact_orders+", "--exclude", "mart_orders"),
            sqlbuild_model_contents_by_relative_path={
                "downstream_orders.sql": (
                    'MODEL (description "Test model downstream_orders.");\n\nselect order_id from __dbt_ref("fact_orders")\n'
                ),
                "mart_orders.sql": (
                    'MODEL (description "Test model mart_orders.");\n\nselect order_id from __ref("downstream_orders")\n'
                ),
            },
            expected_sqlbuild_model_names=("downstream_orders",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "downstream_orders"),),
            expected_dbt_selected_unique_ids=("model.analytics.fact_orders",),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=("fact_orders+",),
            expected_dbt_anchor_unique_ids_by_term={
                "fact_orders+": ("model.analytics.fact_orders",),
            },
            expected_dbt_skip_reason=None,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="routes explicit model path selector for real SQLBuild project",
            args=("--select", "path:sql/models/marts"),
            sqlbuild_model_contents_by_relative_path={
                "marts/mart_orders.sql": "MODEL (description 'Test model mart_orders.');\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=("mart_orders",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "mart_orders"),),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=DbtInteropSkipReason.NO_DBT_WORK,
            expected_sqlbuild_skip_reason=None,
            expected_path_translations=(),
        ),
        RealDbtInteropPlanTestCase(
            description="translates backslash model path selector for real SQLBuild project",
            args=("--select", "path:models\\marts"),
            sqlbuild_model_contents_by_relative_path={
                "marts/mart_orders.sql": "MODEL (description 'Test model mart_orders.');\n\nselect 1 as order_id\n",
                "staging/stg_local.sql": "MODEL (description 'Test model stg_local.');\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=("mart_orders",),
            expected_sqlbuild_command_argvs=(("sqb", "plan", "--select", "mart_orders"),),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=DbtInteropSkipReason.NO_DBT_WORK,
            expected_sqlbuild_skip_reason=None,
            expected_path_translations=(("path:models\\marts", "path:sql/models/marts"),),
        ),
        RealDbtInteropPlanTestCase(
            description="preserves routed SQLBuild cursor args in real plan argv",
            args=("--select", "local_only", "--start-cursor-int", "10"),
            sqlbuild_model_contents_by_relative_path={
                "local_only.sql": "MODEL (description 'Test model local_only.');\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=("local_only",),
            expected_sqlbuild_command_argvs=(
                ("sqb", "plan", "--select", "local_only", "--start-cursor-int", "10"),
            ),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=DbtInteropSkipReason.NO_DBT_WORK,
            expected_sqlbuild_skip_reason=None,
        ),
        RealDbtInteropPlanTestCase(
            description="returns stable no-work plan for unmatched selectors",
            args=("--select", "does_not_exist"),
            sqlbuild_model_contents_by_relative_path={
                "local_only.sql": "MODEL (description 'Test model local_only.');\n\nselect 1 as order_id\n",
            },
            expected_sqlbuild_model_names=(),
            expected_sqlbuild_command_argvs=(),
            expected_dbt_selected_unique_ids=(),
            expected_dbt_required_unique_ids=(),
            expected_dbt_required_selector_terms=(),
            expected_supplemental_dbt_command_argvs=(),
            expected_dbt_anchor_terms=(),
            expected_dbt_anchor_unique_ids_by_term={},
            expected_dbt_skip_reason=DbtInteropSkipReason.NO_DBT_WORK,
            expected_sqlbuild_skip_reason=DbtInteropSkipReason.NO_SQLBUILD_WORK,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_real_dbt_and_sqlbuild_project_when_planning_then_returns_expected_plan(
    test_case: RealDbtInteropPlanTestCase,
    real_dbt_executable: str,
    dbt_project_dir: Path,
    dbt_profiles_dir: Path,
    tmp_path: Path,
) -> None:
    dbt_target_path: Path = dbt_project_dir / "target"
    sqlbuild_project_dir: Path = build_sqlbuild_project_with_dbt_config(
        tmp_path=tmp_path,
        dbt_project_dir=dbt_project_dir,
        dbt_profiles_dir=dbt_profiles_dir,
        dbt_target_path=dbt_target_path,
        model_contents_by_relative_path=test_case.sqlbuild_model_contents_by_relative_path,
    )

    plan: DbtInteropPlan = plan_dbt_interop_from_project(
        project_dir=sqlbuild_project_dir,
        args=test_case.args,
        dbt_executable=real_dbt_executable,
    )

    assert plan.selection.sqlbuild_model_names == test_case.expected_sqlbuild_model_names
    assert plan.sqlbuild_command_argvs == test_case.expected_sqlbuild_command_argvs
    assert plan.dbt_selected_unique_ids == test_case.expected_dbt_selected_unique_ids
    assert plan.selection.dbt_required_unique_ids == test_case.expected_dbt_required_unique_ids
    assert plan.dbt_required_selector_terms == test_case.expected_dbt_required_selector_terms
    assert plan.supplemental_dbt_command_argvs == resolve_expected_dbt_argvs(
        test_case.expected_supplemental_dbt_command_argvs,
        dbt_project_dir=dbt_project_dir,
        dbt_profiles_dir=dbt_profiles_dir,
        dbt_target_path=dbt_target_path,
    )
    assert plan.selection.dbt_anchor_terms == test_case.expected_dbt_anchor_terms
    assert (
        plan.selection.dbt_anchor_unique_ids_by_term
        == test_case.expected_dbt_anchor_unique_ids_by_term
    )
    assert plan.dbt_skip_reason == test_case.expected_dbt_skip_reason
    assert plan.sqlbuild_skip_reason == test_case.expected_sqlbuild_skip_reason
    assert plan.selection.path_translations == test_case.expected_path_translations
