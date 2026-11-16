use crate::tests::helpers::install_test_identity;
use serde_json::json;

use crate::python_rules::tests::helpers::{owned_faults, python_rule_faults};
use crate::python_rules::tests::test_types::PythonRunTestCase;

const ORCHESTRATION_RULES: [&str; 5] = [
    "SQBRPYTHON016",
    "SQBRPYTHON017",
    "SQBRPYTHON018",
    "SQBRPYTHON019",
    "SQBRPYTHON020",
];

#[test]
fn given_sensor_and_calendar_modules_when_evaluating_orchestration_rules_then_reports_each_broken_convention()
 {
    install_test_identity();
    let test_cases = [
        PythonRunTestCase {
            description: "domain folders with one declaration per module, private support and allowed imports report nothing",
            files: &[
                ("orchestration/sensors/__init__.py", ""),
                (
                    "orchestration/sensors/inventory/stock_feed.py",
                    r#"from sqlbuild.orchestration import sensor

from orchestration.sensors.inventory._constants import STALE_AFTER
from python.utils.retries import ATTEMPTS
from providers.carrier_api.provider import CarrierApi

from ._helpers.feed_age import feed_age

_LIMIT: int = 3


@sensor("stock_feed_stale_sensor", description="Stale feed.", run="restock_job")
def stock_feed_stale(ctx, carrier_api: CarrierApi):
    return feed_age(ctx) > STALE_AFTER * ATTEMPTS * _LIMIT
"#,
                ),
                (
                    "orchestration/sensors/inventory/_constants.py",
                    "STALE_AFTER = 3\n",
                ),
                (
                    "orchestration/sensors/inventory/_helpers/feed_age.py",
                    "def feed_age(ctx):\n    return 0\n",
                ),
                (
                    "orchestration/calendars/finance/month_end.py",
                    r#"from sqlbuild.orchestration import calendar

month_end = calendar("month_end_calendar", description="Month end.")
"#,
                ),
                ("python/utils/retries.py", "ATTEMPTS = 2\n"),
                ("providers/carrier_api/provider.py", ""),
            ],
            config: json!({ "select": ORCHESTRATION_RULES }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "layout: no domain folder, a generic name, nesting, and support outside a domain or not in the allowed files",
            files: &[
                ("orchestration/sensors/stock_feed.py", ""),
                ("orchestration/sensors/inventory/sensors.py", ""),
                ("orchestration/sensors/inventory/sub/feed.py", ""),
                ("orchestration/sensors/inventory/_constants.py", ""),
                ("orchestration/sensors/inventory/_helpers/feed_age.py", ""),
                ("orchestration/sensors/inventory/_utils.py", ""),
                ("orchestration/sensors/inventory/_lib/age.py", ""),
                ("orchestration/sensors/_shared.py", ""),
                ("orchestration/sensors/_shared/age.py", ""),
                ("orchestration/calendars/__init__.py", ""),
            ],
            config: json!({ "select": ["SQBRPYTHON016", "SQBRPYTHON017"] }),
            expected_faults: vec![
                ("SQBRPYTHON017", "orchestration/sensors/_shared.py", 1, 1),
                (
                    "SQBRPYTHON017",
                    "orchestration/sensors/_shared/age.py",
                    1,
                    1,
                ),
                (
                    "SQBRPYTHON017",
                    "orchestration/sensors/inventory/_lib/age.py",
                    1,
                    1,
                ),
                (
                    "SQBRPYTHON017",
                    "orchestration/sensors/inventory/_utils.py",
                    1,
                    1,
                ),
                (
                    "SQBRPYTHON016",
                    "orchestration/sensors/inventory/sensors.py",
                    1,
                    1,
                ),
                (
                    "SQBRPYTHON016",
                    "orchestration/sensors/inventory/sub/feed.py",
                    1,
                    1,
                ),
                ("SQBRPYTHON016", "orchestration/sensors/stock_feed.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "surface: a public constant, a mixed assignment, a function, a class, a loop and a docstring on the sensor",
            files: &[(
                "orchestration/sensors/inventory/stock_feed.py",
                r#""""Stock feed sensor."""

from sqlbuild.orchestration import sensor

_STALE_AFTER = 3
STALE = 4
_A, B.x = 1, 2


def helper():
    pass


class Feed:
    pass


@sensor("stock_feed_stale_sensor", description="Stale feed.", run="restock_job")
def stock_feed_stale(ctx):
    """Doc."""
    return None


for row in []:
    pass
"#,
            )],
            config: json!({ "select": ["SQBRPYTHON018", "SQBRPYTHON020"] }),
            expected_faults: vec![
                (
                    "SQBRPYTHON018",
                    "orchestration/sensors/inventory/stock_feed.py",
                    6,
                    1,
                ),
                (
                    "SQBRPYTHON018",
                    "orchestration/sensors/inventory/stock_feed.py",
                    7,
                    1,
                ),
                (
                    "SQBRPYTHON018",
                    "orchestration/sensors/inventory/stock_feed.py",
                    10,
                    5,
                ),
                (
                    "SQBRPYTHON018",
                    "orchestration/sensors/inventory/stock_feed.py",
                    14,
                    7,
                ),
                (
                    "SQBRPYTHON020",
                    "orchestration/sensors/inventory/stock_feed.py",
                    19,
                    5,
                ),
                (
                    "SQBRPYTHON018",
                    "orchestration/sensors/inventory/stock_feed.py",
                    24,
                    1,
                ),
            ],
        },
        PythonRunTestCase {
            description: "imports: another domain's support, a node unit from a sensor or its helpers, and orchestration/ from outside it except providers and node units",
            files: &[
                (
                    "orchestration/sensors/orders/refund_spike.py",
                    r#"from orchestration.sensors.inventory._helpers.feed_age import feed_age
from python.orders.daily.main import main
from python.utils.retries import ATTEMPTS
from providers.carrier_api.provider import CarrierApi
from ._helpers.window import window
"#,
                ),
                ("orchestration/sensors/orders/_helpers/window.py", ""),
                ("orchestration/sensors/inventory/_helpers/feed_age.py", ""),
                (
                    "orchestration/sensors/inventory/_helpers/age.py",
                    "from ...orders._helpers.window import window\nfrom python.orders.daily.main import main\n",
                ),
                ("python/orders/weekly/node.py", ""),
                (
                    "python/orders/weekly/main.py",
                    "from orchestration.sensors.orders._helpers.window import window\n",
                ),
                (
                    "python/orders/daily/main.py",
                    "from orchestration.sensors.orders._helpers.window import window\n",
                ),
                ("python/utils/retries.py", ""),
                (
                    "providers/carrier_api/provider.py",
                    "import orchestration.sensors.orders.refund_spike\n",
                ),
                (
                    "hooks/python/notify.py",
                    "import orchestration.sensors.orders.refund_spike\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON008", "SQBRPYTHON017", "SQBRPYTHON019"] }),
            expected_faults: vec![
                ("SQBRPYTHON019", "hooks/python/notify.py", 1, 8),
                (
                    "SQBRPYTHON017",
                    "orchestration/sensors/inventory/_helpers/age.py",
                    1,
                    39,
                ),
                (
                    "SQBRPYTHON019",
                    "orchestration/sensors/inventory/_helpers/age.py",
                    2,
                    38,
                ),
                (
                    "SQBRPYTHON017",
                    "orchestration/sensors/orders/refund_spike.py",
                    1,
                    63,
                ),
                (
                    "SQBRPYTHON019",
                    "orchestration/sensors/orders/refund_spike.py",
                    2,
                    38,
                ),
                ("SQBRPYTHON019", "python/orders/daily/main.py", 1, 58),
                ("SQBRPYTHON008", "python/orders/weekly/main.py", 1, 58),
            ],
        },
    ];
    for test_case in test_cases {
        let faults = python_rule_faults(test_case.files, test_case.config.clone());

        assert_eq!(
            faults,
            Ok(owned_faults(&test_case.expected_faults)),
            "{}",
            test_case.description
        );
    }
}
