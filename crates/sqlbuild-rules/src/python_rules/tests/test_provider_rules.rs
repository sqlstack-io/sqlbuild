use serde_json::json;

use crate::python_rules::tests::helpers::{owned_faults, python_rule_faults};
use crate::python_rules::tests::test_types::PythonRunTestCase;

const PROVIDER_RULES: [&str; 3] = ["SQBRPYTHON013", "SQBRPYTHON014", "SQBRPYTHON015"];

#[test]
fn given_providers_when_evaluating_provider_rules_then_reports_each_broken_convention() {
    let test_cases = [
        PythonRunTestCase {
            description: "capability folders with provider.py, role files and private folders report nothing",
            files: &[
                ("providers/__init__.py", ""),
                (
                    "providers/storefront_api/provider.py",
                    r#""""Storefront client."""
from sqlbuild.providers import Provider

from python.utils.retries import ATTEMPTS

from ._classes.page_cursor import PageCursor

_PAGE_SIZE: int = 100


class StorefrontApi(Provider):
    """Read-only client for the storefront order API."""

    base_url: str
"#,
                ),
                ("providers/storefront_api/models.py", ""),
                ("providers/storefront_api/_classes/page_cursor.py", ""),
                ("providers/storefront_api/_helpers/paging.py", ""),
                ("python/utils/retries.py", ""),
                (
                    "python/orders/daily/main.py",
                    "from providers.storefront_api.provider import StorefrontApi\n",
                ),
            ],
            config: json!({ "select": PROVIDER_RULES }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "layout: a module directly in providers/, a stray module, a missing provider.py and a stray folder",
            files: &[
                ("providers/storefront_api.py", ""),
                ("providers/carrier_api/client.py", ""),
                ("providers/partner_sftp/provider.py", ""),
                ("providers/partner_sftp/sftp/connection.py", ""),
            ],
            config: json!({ "select": ["SQBRPYTHON013"] }),
            expected_faults: vec![
                ("SQBRPYTHON013", "providers/carrier_api/client.py", 1, 1),
                ("SQBRPYTHON013", "providers/carrier_api/client.py", 1, 1),
                ("SQBRPYTHON013", "providers/partner_sftp/sftp/connection.py", 1, 1),
                ("SQBRPYTHON013", "providers/storefront_api.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "provider.py: a public constant, a function, a private and a plain class, a second provider, and none at all",
            files: &[
                (
                    "providers/storefront_api/provider.py",
                    r#""""Doc."""
from sqlbuild.providers import Provider

_TIMEOUT: int = 3
RETRIES = 2


def build() -> None:
    pass


class _Cursor:
    pass


class Helper:
    pass


class StorefrontApi(Provider):
    """Client."""


class CarrierApi(Provider):
    """Second."""
"#,
                ),
                ("providers/empty/provider.py", "import os\n"),
            ],
            config: json!({ "select": ["SQBRPYTHON014"] }),
            expected_faults: vec![
                ("SQBRPYTHON014", "providers/empty/provider.py", 1, 1),
                ("SQBRPYTHON014", "providers/storefront_api/provider.py", 5, 1),
                ("SQBRPYTHON014", "providers/storefront_api/provider.py", 8, 5),
                ("SQBRPYTHON014", "providers/storefront_api/provider.py", 12, 7),
                ("SQBRPYTHON014", "providers/storefront_api/provider.py", 16, 7),
                ("SQBRPYTHON014", "providers/storefront_api/provider.py", 24, 7),
            ],
        },
        PythonRunTestCase {
            description: "imports: another provider's private code from anywhere, and python/ outside utils/ or orchestration/ from a provider",
            files: &[
                (
                    "providers/storefront_api/provider.py",
                    r#"from python.utils.retries import ATTEMPTS
from python.orders.daily.main import main
from orchestration.sensors.orders import on_orders
from providers.carrier_api._helpers.auth import token
from ._classes.cursor import Cursor
from providers.carrier_api.provider import CarrierApi
"#,
                ),
                ("providers/storefront_api/_classes/cursor.py", ""),
                ("providers/carrier_api/provider.py", ""),
                ("providers/carrier_api/_helpers/auth.py", ""),
                ("python/utils/retries.py", ""),
                (
                    "python/orders/daily/main.py",
                    "from providers.storefront_api._classes.cursor import Cursor\n",
                ),
                ("orchestration/sensors/orders.py", ""),
                ("hooks/python/notify.py", "import providers.carrier_api._helpers.auth\n"),
            ],
            config: json!({ "select": ["SQBRPYTHON015"] }),
            expected_faults: vec![
                ("SQBRPYTHON015", "hooks/python/notify.py", 1, 8),
                ("SQBRPYTHON015", "providers/storefront_api/provider.py", 2, 38),
                ("SQBRPYTHON015", "providers/storefront_api/provider.py", 3, 42),
                ("SQBRPYTHON015", "providers/storefront_api/provider.py", 4, 49),
                ("SQBRPYTHON015", "python/orders/daily/main.py", 1, 54),
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
