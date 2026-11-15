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
                (
                    "SQBRPYTHON013",
                    "providers/partner_sftp/sftp/connection.py",
                    1,
                    1,
                ),
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
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    5,
                    1,
                ),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    8,
                    5,
                ),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    12,
                    7,
                ),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    16,
                    7,
                ),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    24,
                    7,
                ),
            ],
        },
        PythonRunTestCase {
            description: "provider.py: Provider through an alias, a module attribute or _classes/ bases reached by relative imports; bases outside the parsed sources stay silent",
            files: &[
                (
                    "providers/storefront_api/provider.py",
                    r#"from ._classes.paged import PagedApi


class StorefrontApi(PagedApi):
    """Client."""
"#,
                ),
                (
                    "providers/storefront_api/_classes/paged.py",
                    "from .base import BaseApi\n\n\nclass PagedApi(BaseApi):\n    pass\n",
                ),
                (
                    "providers/storefront_api/_classes/base.py",
                    "from sqlbuild.providers import Provider\n\n\nclass BaseApi(Provider):\n    pass\n",
                ),
                (
                    "providers/carrier_api/provider.py",
                    r#"from sqlbuild.providers import Provider as SqbProvider


class CarrierApi(SqbProvider):
    """Client."""
"#,
                ),
                (
                    "providers/billing_api/provider.py",
                    "from ._classes import paged\n\n\nclass BillingApi(paged.PagedApi):\n    \"\"\"Client.\"\"\"\n",
                ),
                (
                    "providers/billing_api/_classes/paged.py",
                    "from sqlbuild.providers import Provider\n\n\nclass PagedApi(Provider):\n    pass\n",
                ),
                (
                    "providers/ledger_api/provider.py",
                    "from ._classes import LedgerBase\n\n\nclass LedgerApi(LedgerBase):\n    \"\"\"Client.\"\"\"\n",
                ),
                (
                    "providers/ledger_api/_classes/__init__.py",
                    "from .base import LedgerBase\n",
                ),
                (
                    "providers/ledger_api/_classes/base.py",
                    "from sqlbuild.providers import Provider\n\n\nclass LedgerBase(Provider):\n    pass\n",
                ),
                (
                    "providers/catalog_api/provider.py",
                    "from python.utils.clients import PagedClient\n\n\nclass CatalogApi(PagedClient):\n    \"\"\"Client.\"\"\"\n",
                ),
                (
                    "python/utils/clients.py",
                    "from python.utils.base import BaseClient\n\n\nclass PagedClient(BaseClient):\n    pass\n",
                ),
                (
                    "python/utils/base.py",
                    "from sqlbuild.providers import Provider\n\n\nclass BaseClient(Provider):\n    pass\n",
                ),
                (
                    "providers/partner_api/provider.py",
                    "from vendor.clients import VendorProvider\n\n\nclass PartnerApi(VendorProvider):\n    \"\"\"Client.\"\"\"\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON014"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "provider.py: tuple, subscript, mixed and chained assignments, a builtin base and a module attribute naming a plain class",
            files: &[
                (
                    "providers/storefront_api/provider.py",
                    r#"import os

from sqlbuild.providers import Provider

_HOST, _PORT = "h", 1
HOST, _TIMEOUT = "h", 3
os.environ["TZ"] = "UTC"
_ZONE, os.environ["TZ"] = "h", "UTC"
_TZ = os.environ["TZ"] = "UTC"


class StorefrontApi(Provider):
    """Client."""
"#,
                ),
                (
                    "providers/carrier_api/provider.py",
                    "from ._classes import shapes\n\n\nclass CarrierApi(shapes.Parcel):\n    \"\"\"Client.\"\"\"\n",
                ),
                (
                    "providers/carrier_api/_classes/shapes.py",
                    "class Parcel:\n    pass\n",
                ),
                (
                    "providers/returns_api/provider.py",
                    "class ReturnsApi(Exception):\n    \"\"\"Client.\"\"\"\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON014"] }),
            expected_faults: vec![
                ("SQBRPYTHON014", "providers/carrier_api/provider.py", 1, 1),
                ("SQBRPYTHON014", "providers/carrier_api/provider.py", 4, 7),
                ("SQBRPYTHON014", "providers/returns_api/provider.py", 1, 1),
                ("SQBRPYTHON014", "providers/returns_api/provider.py", 1, 7),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    6,
                    1,
                ),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    7,
                    1,
                ),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    8,
                    1,
                ),
                (
                    "SQBRPYTHON014",
                    "providers/storefront_api/provider.py",
                    9,
                    1,
                ),
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
                (
                    "hooks/python/notify.py",
                    "import providers.carrier_api._helpers.auth\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON015"] }),
            expected_faults: vec![
                ("SQBRPYTHON015", "hooks/python/notify.py", 1, 8),
                (
                    "SQBRPYTHON015",
                    "providers/storefront_api/provider.py",
                    2,
                    38,
                ),
                (
                    "SQBRPYTHON015",
                    "providers/storefront_api/provider.py",
                    3,
                    42,
                ),
                (
                    "SQBRPYTHON015",
                    "providers/storefront_api/provider.py",
                    4,
                    49,
                ),
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
