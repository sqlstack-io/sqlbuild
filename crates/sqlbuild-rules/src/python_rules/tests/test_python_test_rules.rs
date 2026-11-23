use crate::tests::helpers::install_test_identity;
use serde_json::json;

use crate::python_rules::tests::helpers::{owned_faults, python_rule_faults};
use crate::python_rules::tests::test_types::PythonRunTestCase;

const RUNTIME_FILES: [(&str, &str); 5] = [
    (
        "python/nodes/orders/storefront_import/asset.py",
        r#"from sqlbuild.assets import AssetContext, asset

from python.nodes.orders.storefront_import.main import main


@asset(description="Import storefront orders.")
def storefront_import(ctx: AssetContext) -> None:
    return main(ctx)
"#,
    ),
    (
        "python/nodes/orders/storefront_import/main.py",
        r#"from sqlbuild.assets import PythonNodeResult, AssetContext


def main(ctx: AssetContext) -> PythonNodeResult:
    return PythonNodeResult()
"#,
    ),
    (
        "python/nodes/orders/storefront_import/_helpers/row_mapping.py",
        r#"def map_row(raw: dict[str, object]) -> dict[str, object]:
    return {"status": str(raw["state"]).lower()}
"#,
    ),
    (
        "python/providers/storefront_api/provider.py",
        r#"from sqlbuild.providers import Provider


class StorefrontApi(Provider):
    region: str = "eu"
"#,
    ),
    ("python/providers/storefront_api/__init__.py", ""),
];

const CASE_TYPES: &str = r#"from dataclasses import dataclass


@dataclass(frozen=True)
class RowMappingCase:
    description: str
    raw: dict[str, object]
    expected_status: str
"#;

const ROW_MAPPING_TEST: &str = r#"import pytest

from python.nodes.orders.storefront_import._helpers.row_mapping import map_row
from tests.python.nodes.orders.storefront_import._helpers._test_types import RowMappingCase


@pytest.mark.parametrize(
    "test_case",
    [
        RowMappingCase(description="paid order", raw={"state": "PAID"}, expected_status="paid"),
    ],
    ids=lambda case: case.description,
)
def test_given_raw_order_when_mapping_then_status_is_normalised(test_case: RowMappingCase) -> None:
    assert map_row(test_case.raw)["status"] == test_case.expected_status
"#;

const PROVIDER_CASE_TYPES: &str = r#"from dataclasses import dataclass


@dataclass(frozen=True)
class RegionCase:
    description: str
    region: str
    expected_region: str
"#;

const PROVIDER_TEST: &str = r#"import pytest

from python.providers.storefront_api.provider import StorefrontApi
from tests.python.providers.storefront_api._test_types import RegionCase


@pytest.mark.parametrize(
    "test_case",
    [RegionCase(description="explicit region", region="us", expected_region="us")],
    ids=lambda case: case.description,
)
def test_given_region_when_constructing_then_provider_keeps_it(test_case: RegionCase) -> None:
    assert StorefrontApi(region=test_case.region).region == test_case.expected_region
"#;

const BRANCHING_TEST: &str = r#"import pytest

from .._test_types import RowMappingCase


def build_raw(state: str) -> dict[str, object]:
    return {"state": state}


@pytest.mark.parametrize(
    "test_case",
    [RowMappingCase(description="paid order", raw={"state": "PAID"}, expected_status="paid")],
    ids=lambda case: case.description,
)
def test_given_raw_order_when_mapping_then_status_is_normalised(test_case: RowMappingCase) -> None:
    if test_case.raw:
        assert build_raw("PAID") == test_case.raw
    assert test_case.expected_status == "paid"
"#;

#[test]
fn given_project_python_tests_when_evaluating_test_rules_then_reports_misplaced_and_unconventional_tests()
 {
    install_test_identity();
    let test_cases = [
        PythonRunTestCase {
            description: "tests mirroring a unit, its helpers and a provider, with dataclass cases, report nothing",
            files: &[
                RUNTIME_FILES[0],
                RUNTIME_FILES[1],
                RUNTIME_FILES[2],
                RUNTIME_FILES[3],
                RUNTIME_FILES[4],
                (
                    "tests/python/nodes/orders/storefront_import/_helpers/_test_types.py",
                    CASE_TYPES,
                ),
                (
                    "tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py",
                    ROW_MAPPING_TEST,
                ),
                (
                    "tests/python/providers/storefront_api/_test_types.py",
                    PROVIDER_CASE_TYPES,
                ),
                (
                    "tests/python/providers/storefront_api/test_provider.py",
                    PROVIDER_TEST,
                ),
            ],
            config: json!({"select": ["SQBRPYTHON021", "SQBRPYTHON701", "SQBRPYTHON702", "SQBRPYTHON703", "SQBRPYTHON704", "SQBRPYTHON705", "SQBRPYTHON706", "SQBRPYTHON707", "SQBRPYTHON708", "SQBRPYTHON709", "SQBRPYTHON710", "SQBRPYTHON711", "SQBRPYTHON712", "SQBRPYTHON713", "SQBRPYTHON714", "SQBRPYTHON715", "SQBRPYTHON716", "SQBRPYTHON717", "SQBRPYTHON718", "SQBRPYTHON719", "SQBRPYTHON720", "SQBRPYTHON721", "SQBRPYTHON722", "SQBRPYTHON723", "SQBRPYTHON724"]}),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "tests for missing modules or folders, or outside any folder, fail placement",
            files: &[
                RUNTIME_FILES[0],
                RUNTIME_FILES[1],
                ("tests/python/nodes/orders/_test_types.py", CASE_TYPES),
                (
                    "tests/python/nodes/orders/test_storefront.py",
                    ROW_MAPPING_TEST,
                ),
                ("tests/python/nodes/inventory/_test_types.py", CASE_TYPES),
                (
                    "tests/python/nodes/inventory/test_stock.py",
                    ROW_MAPPING_TEST,
                ),
                ("tests/python/nodes/_test_types.py", CASE_TYPES),
                ("tests/python/nodes/test_everything.py", ROW_MAPPING_TEST),
            ],
            config: json!({"select": ["SQBRPYTHON021"]}),
            expected_faults: vec![
                ("SQBRPYTHON021", "tests/python/nodes/_test_types.py", 1, 1),
                (
                    "SQBRPYTHON021",
                    "tests/python/nodes/inventory/_test_types.py",
                    1,
                    1,
                ),
                (
                    "SQBRPYTHON021",
                    "tests/python/nodes/inventory/test_stock.py",
                    1,
                    1,
                ),
                (
                    "SQBRPYTHON021",
                    "tests/python/nodes/orders/test_storefront.py",
                    1,
                    1,
                ),
                (
                    "SQBRPYTHON021",
                    "tests/python/nodes/test_everything.py",
                    1,
                    1,
                ),
            ],
        },
        PythonRunTestCase {
            description: "an orchestration test mirrors its sensor code; one mirroring nothing is reported",
            files: &[
                (
                    "orchestration/sensors/orders/_helpers/restock_window.py",
                    "def restock_due(hours):\n    return hours >= 6\n",
                ),
                (
                    "tests/orchestration/sensors/orders/_helpers/test_restock_window.py",
                    ROW_MAPPING_TEST,
                ),
                (
                    "tests/orchestration/sensors/inventory/test_stock.py",
                    ROW_MAPPING_TEST,
                ),
            ],
            config: json!({"select": ["SQBRPYTHON021"]}),
            expected_faults: vec![(
                "SQBRPYTHON021",
                "tests/orchestration/sensors/inventory/test_stock.py",
                1,
                1,
            )],
        },
        PythonRunTestCase {
            description: "a relative import, a top-level helper and a branch in a test break the test conventions",
            files: &[
                RUNTIME_FILES[0],
                RUNTIME_FILES[1],
                RUNTIME_FILES[2],
                (
                    "tests/python/nodes/orders/storefront_import/_test_types.py",
                    CASE_TYPES,
                ),
                (
                    "tests/python/nodes/orders/storefront_import/_helpers/_test_types.py",
                    CASE_TYPES,
                ),
                (
                    "tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py",
                    BRANCHING_TEST,
                ),
            ],
            config: json!({"select": ["SQBRPYTHON702", "SQBRPYTHON703", "SQBRPYTHON704"]}),
            expected_faults: vec![
                (
                    "SQBRPYTHON702",
                    "tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py",
                    3,
                    1,
                ),
                (
                    "SQBRPYTHON703",
                    "tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py",
                    6,
                    1,
                ),
                (
                    "SQBRPYTHON704",
                    "tests/python/nodes/orders/storefront_import/_helpers/test_row_mapping.py",
                    16,
                    5,
                ),
            ],
        },
    ];
    for test_case in test_cases {
        let faults = python_rule_faults(test_case.files, test_case.config).expect("rules run");
        assert_eq!(
            faults,
            owned_faults(&test_case.expected_faults),
            "{}",
            test_case.description
        );
    }
}
