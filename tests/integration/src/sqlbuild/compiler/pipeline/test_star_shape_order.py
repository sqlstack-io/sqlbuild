"""Semantic shapes publish a star model only after the upstream shapes it expands."""

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.pipeline._test_types import StarShapeOrderTestCase
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import postgres_semantic_shapes


@pytest.mark.parametrize(
    "test_case",
    [
        StarShapeOrderTestCase(
            description="quoted upstream column keeps its quoting through a star listed first",
            files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "postgres"\n',
                "models/a_orders_wide.sql": (
                    "MODEL (description 'Every order column.', schema analytics);\n"
                    'SELECT * FROM __ref("z_orders")\n'
                ),
                "models/z_orders.sql": (
                    "MODEL (description 'Orders.', schema analytics);\n"
                    "SELECT 1 AS \"OrderId\", 'placed' AS status\n"
                ),
            },
            star_model="a_orders_wide",
            expected_shape={"status": "TEXT", '"OrderId"': "INT"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_star_model_before_its_upstream_when_shaping_then_upstream_shape_is_used(
    test_case: StarShapeOrderTestCase, tmp_path: Path
) -> None:
    shapes: dict[str, dict[str, str]] = postgres_semantic_shapes(
        project_dir=tmp_path, files=test_case.files
    )

    assert shapes[test_case.star_model] == test_case.expected_shape
