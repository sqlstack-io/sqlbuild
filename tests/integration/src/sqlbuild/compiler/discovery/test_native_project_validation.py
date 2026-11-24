"""Native cross-file validation raises the discovery errors Python raised, with their text."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    CsvDialectTestCase,
    ProjectValidationTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    VALIDATION_MODEL,
    csv_reader_error,
    discovery_error,
    seed_project_files,
    stage_outcome,
    write_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectValidationTestCase(
            description="a seed header that differs from its declared columns",
            files=seed_project_files(header=b"code,name"),
            expected_error_type="SeedDiscoveryError",
            expected_message=(
                "sql/seeds/countries.csv header ('code', 'name') does not match declared seed "
                "columns ('code', 'label') for 'countries'"
            ),
        ),
        ProjectValidationTestCase(
            description="a quoted seed header honours the declared quotechar",
            files=seed_project_files(
                header=b'"code";"label";"extra"',
                settings=b"    csv_settings:\n      delimiter: ';'\n      quotechar: '\"'\n",
            ),
            expected_error_type="SeedDiscoveryError",
            expected_message=(
                "sql/seeds/countries.csv header ('code', 'label', 'extra') does not match declared "
                "seed columns ('code', 'label') for 'countries'"
            ),
        ),
        ProjectValidationTestCase(
            description="a seed CSV without a declaration",
            files=(VALIDATION_MODEL, ("sql/seeds/regions.csv", b"code\nEU\n")),
            expected_error_type="SeedDiscoveryError",
            expected_message=(
                "Seed CSV sql/seeds/regions.csv has no matching declaration for seed 'regions' under "
                "sql/seeds/**/*.yml"
            ),
        ),
        ProjectValidationTestCase(
            description="two model files with one name",
            files=(
                VALIDATION_MODEL,
                ("sql/models/marts/orders.sql", b"MODEL ();\nSELECT 2 AS order_id"),
            ),
            expected_error_type="DiscoveryConflictError",
            expected_message=(
                "Duplicate model file name found for 'orders' in sql/models/marts/orders.sql and "
                "sql/models/orders.sql"
            ),
        ),
        ProjectValidationTestCase(
            description="a non-canonical model identity",
            files=(("sql/models/Orders.sql", b"MODEL ();\nSELECT 1 AS order_id"),),
            expected_error_type="ResourceIdentityError",
            expected_message=(
                "Invalid model identity 'Orders' in sql/models/Orders.sql; use snake_case 'orders'"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_conflicting_project_files_when_discovering_then_raises_the_discovery_error(
    test_case: ProjectValidationTestCase, tmp_path: Path
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)

    outcome: tuple[object, ...] = stage_outcome(project_dir=tmp_path)

    assert outcome[1:] == (test_case.expected_error_type, test_case.expected_message)


@pytest.mark.parametrize(
    "test_case",
    [
        CsvDialectTestCase(
            description="an empty quotechar",
            settings=b"    csv_settings:\n      quotechar: ''\n",
            expected_reader_settings={"quotechar": ""},
        ),
        CsvDialectTestCase(
            description="a delimiter equal to its quotechar",
            settings=b"    csv_settings:\n      delimiter: ','\n      quotechar: ','\n",
            expected_reader_settings={"quotechar": ","},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seed_csv_settings_when_discovering_then_matches_this_pythons_csv_reader(
    test_case: CsvDialectTestCase, tmp_path: Path
) -> None:
    write_project(
        project_dir=tmp_path,
        files=seed_project_files(header=b"code,label", settings=test_case.settings),
    )

    outcome: tuple[str, str] | None = discovery_error(project_dir=tmp_path)

    assert outcome == csv_reader_error(test_case.expected_reader_settings)
