from pathlib import Path

import pytest

from scripts.native_layering._helpers.layering import get_native_layering_errors
from tests.unit.scripts.native_layering._helpers._test_types import (
    NativeLayeringTestCase,
    RepositoryLayeringTestCase,
)
from tests.unit.scripts.native_layering._helpers.helpers import write_workspace

ORDER: tuple[str, ...] = (
    "sqlbuild-core",
    "sqlbuild-sqltext",
    "sqlbuild-analysis",
    "sqlbuild-python-base",
    "sqlbuild-python",
)
LAYERED: dict[str, tuple[str, ...]] = {
    "sqlbuild-core": (),
    "sqlbuild-sqltext": ("sqlbuild-core",),
    "sqlbuild-analysis": ("sqlbuild-core", "sqlbuild-sqltext", "polyglot-sql"),
    "sqlbuild-python-base": ("sqlbuild-analysis", "pyo3"),
    "sqlbuild-python": ("sqlbuild-python-base", "pyo3"),
}
EXTERNAL: dict[str, tuple[str, ...]] = {
    "polyglot-sql": ("serde",),
    "pyo3": ("pyo3-ffi",),
    "pyo3-ffi": (),
    "serde": (),
}


@pytest.mark.parametrize(
    "test_case",
    [
        NativeLayeringTestCase(
            description="one-way layers with PyO3 only in the listed Python crates",
            crate_dependencies=LAYERED,
            external_dependencies=EXTERNAL,
            order=ORDER,
            expected_errors=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_layered_workspace_when_checking_then_returns_no_errors(
    tmp_path: Path, test_case: NativeLayeringTestCase
) -> None:
    write_workspace(tmp_path, test_case)

    assert get_native_layering_errors(tmp_path) == test_case.expected_errors


@pytest.mark.parametrize(
    "test_case",
    [
        NativeLayeringTestCase(
            description="lower crate depends on a later layer",
            crate_dependencies={**LAYERED, "sqlbuild-core": ("sqlbuild-sqltext",)},
            external_dependencies=EXTERNAL,
            order=ORDER,
            expected_errors=(
                "sqlbuild-core depends on sqlbuild-sqltext, which is not in an earlier layer.",
            ),
        ),
        NativeLayeringTestCase(
            description="analysis reaches PyO3 through an external crate",
            crate_dependencies={
                **LAYERED,
                "sqlbuild-analysis": ("sqlbuild-core", "polyglot-sql", "python-bridge"),
            },
            external_dependencies={**EXTERNAL, "python-bridge": ("pyo3",)},
            order=ORDER,
            expected_errors=(
                "sqlbuild-analysis depends on pyo3; only sqlbuild-python-base, sqlbuild-python may.",
            ),
        ),
        NativeLayeringTestCase(
            description="crate below the polyglot floor uses polyglot",
            crate_dependencies={**LAYERED, "sqlbuild-sqltext": ("sqlbuild-core", "polyglot-sql")},
            external_dependencies=EXTERNAL,
            order=ORDER,
            expected_errors=(
                "sqlbuild-sqltext depends on polyglot-sql, which starts at sqlbuild-analysis.",
            ),
        ),
        NativeLayeringTestCase(
            description="aliased workspace dependency points upward",
            crate_dependencies={**LAYERED, "sqlbuild-core": ("text",)},
            external_dependencies=EXTERNAL,
            order=ORDER,
            workspace_aliases={"text": "sqlbuild-sqltext"},
            expected_errors=(
                "sqlbuild-core depends on sqlbuild-sqltext, which is not in an earlier layer.",
            ),
        ),
        NativeLayeringTestCase(
            description="workspace crate missing from the layer order",
            crate_dependencies={**LAYERED, "sqlbuild-extra": ("sqlbuild-core",)},
            external_dependencies=EXTERNAL,
            order=ORDER,
            expected_errors=(
                "Workspace crate sqlbuild-extra is missing from the native layer order.",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_layering_violation_when_checking_then_reports_it(
    tmp_path: Path, test_case: NativeLayeringTestCase
) -> None:
    write_workspace(tmp_path, test_case)

    assert get_native_layering_errors(tmp_path) == test_case.expected_errors


@pytest.mark.parametrize(
    "test_case",
    [
        RepositoryLayeringTestCase(
            description="repository crates follow their declared layers",
            expected_errors=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_repository_workspace_when_checking_then_layers_hold(
    test_case: RepositoryLayeringTestCase,
) -> None:
    repository: Path = Path(__file__).resolve().parents[5]

    assert get_native_layering_errors(repository) == test_case.expected_errors


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
