"""Header values at the nesting limit project identically on a deep Python stack."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import parse_header_values
from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    DeepHeaderValuesTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    deep_model_header_values,
    on_deep_stack,
    value_shape,
    write_project,
)

_AT_LIMIT_HEADER: str = (
    "\n  description 'Orders.',\n"
    f"  tags {'[' * 256}'placed'{']' * 256},\n"
    f"  config {'(level ' * 256}1{')' * 256},\n"
    f"  placeholders {'call(level ' * 128}1{')' * 128},\n"
    f"  audits {'{' * 256}1{'}' * 256},\n"
    f"  row_diff_tolerances {'constant(value ' * 256}1{')' * 256},\n"
    f"  post_hooks [{'sql("refresh", level: ' * 127}1{')' * 127}],\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        DeepHeaderValuesTestCase(
            description="every container kind at the limit beneath 300 extra frames",
            header=_AT_LIMIT_HEADER,
            stack_frames=300,
            expected_max_depth=769,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_header_values_at_the_limit_when_discovering_on_a_deep_stack_then_values_match_the_parser(
    test_case: DeepHeaderValuesTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(
        project_dir=tmp_path,
        files=(
            ("sql/models/orders.sql", f"MODEL ({test_case.header});\nSELECT 1 AS id\n".encode()),
        ),
    )
    parsed: object = on_deep_stack(
        frames=test_case.stack_frames,
        call=lambda: parse_header_values(
            header=test_case.header,
            file_path=tmp_path / "sql/models/orders.sql",
            statement_name="MODEL",
            header_line=1,
        ),
    )

    shape: tuple[tuple[int, str], ...] = value_shape(
        deep_model_header_values(project_dir=tmp_path, frames=test_case.stack_frames)
    )

    assert shape == value_shape(parsed)
    assert max(depth for depth, _ in shape) == test_case.expected_max_depth


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
