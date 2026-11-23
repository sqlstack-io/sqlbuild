use crate::tests::helpers::install_test_identity;
use serde_json::json;

use crate::python_rules::tests::helpers::{owned_faults, python_rule_faults};
use crate::python_rules::tests::test_types::PythonRunTestCase;

const UNIT_RULES: [&str; 10] = [
    "SQBRPYTHON003",
    "SQBRPYTHON004",
    "SQBRPYTHON005",
    "SQBRPYTHON006",
    "SQBRPYTHON007",
    "SQBRPYTHON008",
    "SQBRPYTHON009",
    "SQBRPYTHON010",
    "SQBRPYTHON011",
    "SQBRPYTHON012",
];

const DAILY_ASSET: &str = r#"from .main import main


@asset(description="Daily.")
def daily(ctx: AssetContext) -> None:
    return main(ctx=ctx)
"#;

const ASSET_MAIN: &str = r#"from sqlbuild import PythonNodeResult


def main(ctx: AssetContext) -> PythonNodeResult:
    return ctx.result()
"#;

#[test]
fn given_node_units_when_evaluating_unit_rules_then_reports_each_broken_convention() {
    install_test_identity();
    let test_cases = [
        PythonRunTestCase {
            description: "a project laid out in units reports nothing",
            files: &[
                ("python/providers/store/provider.py", "class Store:\n    pass\n"),
                (
                    "python/nodes/utils/text.py",
                    "def clean() -> str:\n    return ''\n",
                ),
                ("python/nodes/orders/constants.py", "LIMIT: int = 3\n"),
                (
                    "python/nodes/orders/_helpers/dates.py",
                    "def today() -> str:\n    return ''\n",
                ),
                (
                    "python/nodes/orders/daily/asset.py",
                    r#"from python.providers.store.provider import Store

from .main import main


@asset(description="Daily.")
def daily(ctx: AssetContext, *, store: Store) -> None:
    return main(ctx=ctx, store=store)
"#,
                ),
                (
                    "python/nodes/orders/daily/main.py",
                    r#""""Daily totals."""
from sqlbuild import PythonNodeResult, PythonNodeSkipResult

from python.providers.store.provider import Store
from python.nodes.orders._helpers.dates import today
from python.nodes.orders.constants import LIMIT
from python.nodes.utils.text import clean

from ._helpers.rows import rows


def main(ctx: AssetContext, store: Store) -> PythonNodeResult | PythonNodeSkipResult:
    return ctx.result(rows(today(), clean(), LIMIT, store))
"#,
                ),
                (
                    "python/nodes/orders/daily/_helpers/rows.py",
                    "def rows() -> list:\n    return []\n",
                ),
                (
                    "python/nodes/orders/daily/checks.py",
                    r#"from .asset import daily


@check(depends_on=[daily], description="Totals are positive.")
def totals_positive(ctx: CheckContext) -> bool:
    return ctx.ok()
"#,
                ),
                (
                    "python/nodes/orders/weekly/asset.py",
                    r#"from python.nodes.orders.daily.asset import daily
from python.nodes.orders.weekly.main import main


@asset(depends_on=[daily], description="Weekly.")
def weekly(ctx: AssetContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/nodes/orders/weekly/main.py",
                    "def main(ctx: AssetContext) -> PythonNodeResult:\n    return ctx.result()\n",
                ),
                (
                    "python/nodes/orders/raw/loader.py",
                    r#"from .main import main


@loader
def raw(ctx: LoaderContext) -> list:
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/nodes/orders/raw/main.py",
                    "def main(ctx: LoaderContext) -> Optional[Iterator[dict]]:\n    return None\n",
                ),
                (
                    "python/nodes/orders/regions/factory.py",
                    r#"from .constants import REGIONS
from .main import main


@factory
def region_exports():
    for region in REGIONS:

        @asset(name=f"export_{region}", description="Export one region.")
        def export(ctx: AssetContext) -> None:
            return main(ctx=ctx, region=region)

        @check(name=f"export_{region}_ok", depends_on=[export], description="Exported.")
        def exported(ctx: CheckContext) -> bool:
            return True
"#,
                ),
                (
                    "python/nodes/orders/regions/constants.py",
                    "REGIONS: tuple = ('eu', 'us')\n",
                ),
                (
                    "python/nodes/orders/regions/main.py",
                    "def main(ctx: AssetContext, region: str) -> PythonNodeResult:\n    return ctx.result()\n",
                ),
            ],
            config: json!({ "select": UNIT_RULES }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "unit layout: a missing main.py, a stray module and a folder that is not _helpers/",
            files: &[
                ("python/nodes/orders/daily/asset.py", DAILY_ASSET),
                ("python/nodes/orders/weekly/asset.py", ""),
                ("python/nodes/orders/weekly/main.py", ""),
                ("python/nodes/orders/weekly/report.py", ""),
                ("python/nodes/orders/weekly/sql/query.py", ""),
                ("python/nodes/orders/weekly/_helpers/rows.py", ""),
            ],
            config: json!({ "select": ["SQBRPYTHON003"] }),
            expected_faults: vec![
                ("SQBRPYTHON003", "python/nodes/orders/daily/asset.py", 1, 1),
                ("SQBRPYTHON003", "python/nodes/orders/weekly/report.py", 1, 1),
                ("SQBRPYTHON003", "python/nodes/orders/weekly/sql/query.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "asset.py: statements, helpers and a second declaration report; an empty asset.py reports at its start",
            files: &[
                (
                    "python/nodes/orders/daily/asset.py",
                    r#""""Daily."""
from .main import main

LIMIT = 3


def helper() -> int:
    return 1


@asset(description="Daily.")
def daily(ctx: AssetContext) -> None:
    return main(ctx=ctx)


@asset(description="Extra.")
def extra(ctx: AssetContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/nodes/orders/weekly/asset.py", "from .main import main\n"),
            ],
            config: json!({ "select": ["SQBRPYTHON004"] }),
            expected_faults: vec![
                ("SQBRPYTHON004", "python/nodes/orders/daily/asset.py", 4, 1),
                ("SQBRPYTHON004", "python/nodes/orders/daily/asset.py", 7, 5),
                ("SQBRPYTHON004", "python/nodes/orders/daily/asset.py", 17, 5),
                ("SQBRPYTHON004", "python/nodes/orders/weekly/asset.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "delegation: positional arguments, extra statements, unpacking and another unit's main report",
            files: &[
                (
                    "python/nodes/orders/daily/asset.py",
                    r#"from python.nodes.orders.daily.main import main
from python.nodes.orders.weekly.main import main as weekly_main


@asset(description="A.")
def by_keyword(ctx: AssetContext) -> None:
    return main(ctx=ctx)


@asset(description="B.")
def positional(ctx: AssetContext) -> None:
    return main(ctx)


@asset(description="C.")
def computes(ctx: AssetContext) -> None:
    rows = main(ctx=ctx)
    return rows


@asset(description="D.")
def unpacked(ctx: AssetContext) -> None:
    return main(**ctx)
"#,
                ),
                ("python/nodes/orders/daily/main.py", ASSET_MAIN),
                (
                    "python/nodes/orders/weekly/asset.py",
                    r#"from python.nodes.orders.daily.main import main


@asset(description="W.")
def weekly(ctx: AssetContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/nodes/orders/weekly/main.py", ASSET_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON005"] }),
            expected_faults: vec![
                ("SQBRPYTHON005", "python/nodes/orders/daily/asset.py", 11, 5),
                ("SQBRPYTHON005", "python/nodes/orders/daily/asset.py", 16, 5),
                ("SQBRPYTHON005", "python/nodes/orders/daily/asset.py", 22, 5),
                ("SQBRPYTHON005", "python/nodes/orders/weekly/asset.py", 5, 5),
            ],
        },
        PythonRunTestCase {
            description: "main.py: other statements, a result that does not fit the node kind, and no main()",
            files: &[
                ("python/nodes/orders/daily/asset.py", DAILY_ASSET),
                (
                    "python/nodes/orders/daily/main.py",
                    r#"from sqlbuild import PythonNodeResult

LIMIT = 3


def main(ctx: AssetContext) -> list:
    return []


def helper() -> None:
    pass
"#,
                ),
                ("python/nodes/orders/weekly/asset.py", ""),
                (
                    "python/nodes/orders/weekly/main.py",
                    "def run() -> None:\n    pass\n",
                ),
                ("python/nodes/orders/stock/asset.py", ""),
                ("python/nodes/orders/stock/main.py", "def main(ctx):\n    pass\n"),
                (
                    "python/nodes/orders/raw/loader.py",
                    "from .main import main\n\n\n@loader\ndef raw(ctx: LoaderContext) -> list:\n    return main(ctx=ctx)\n",
                ),
                (
                    "python/nodes/orders/raw/main.py",
                    "def main(ctx: LoaderContext) -> PythonNodeResult:\n    return []\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON006"] }),
            expected_faults: vec![
                ("SQBRPYTHON006", "python/nodes/orders/daily/main.py", 3, 1),
                ("SQBRPYTHON006", "python/nodes/orders/daily/main.py", 6, 5),
                ("SQBRPYTHON006", "python/nodes/orders/daily/main.py", 10, 5),
                ("SQBRPYTHON006", "python/nodes/orders/raw/main.py", 1, 5),
                ("SQBRPYTHON006", "python/nodes/orders/stock/main.py", 1, 5),
                ("SQBRPYTHON006", "python/nodes/orders/weekly/main.py", 1, 1),
                ("SQBRPYTHON006", "python/nodes/orders/weekly/main.py", 1, 5),
            ],
        },
        PythonRunTestCase {
            description: "signature: ctx missing or mistyped, variadics and unannotated providers report; positional-only ctx does not",
            files: &[
                (
                    "python/nodes/orders/daily/asset.py",
                    r#"from .main import main


@asset(description="A.")
def no_ctx(context: AssetContext) -> None:
    return main(ctx=context)


@asset(description="B.")
def wrong_type(ctx: LoaderContext, *, store: Store) -> None:
    return main(ctx=ctx, store=store)


@asset(description="C.")
def loose(ctx: AssetContext, *args, store, **kwargs) -> None:
    return main(ctx=ctx)


@asset(description="D.")
def positional_only(ctx: AssetContext, /) -> None:
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/nodes/orders/daily/checks.py",
                    r#"from .asset import no_ctx


@check(depends_on=[no_ctx], description="Q.")
def quality(ctx: AssetContext) -> bool:
    return True
"#,
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON007"] }),
            expected_faults: vec![
                ("SQBRPYTHON007", "python/nodes/orders/daily/checks.py", 5, 5),
                ("SQBRPYTHON007", "python/nodes/orders/daily/asset.py", 5, 5),
                ("SQBRPYTHON007", "python/nodes/orders/daily/asset.py", 10, 5),
                ("SQBRPYTHON007", "python/nodes/orders/daily/asset.py", 15, 5),
                ("SQBRPYTHON007", "python/nodes/orders/daily/asset.py", 15, 5),
            ],
        },
        PythonRunTestCase {
            description: "imports: another unit's main, role files and helpers, a domain module, and a node used outside depends_on report",
            files: &[
                ("python/providers/store/provider.py", ""),
                ("python/providers/store/client.py", ""),
                ("python/nodes/utils/text.py", ""),
                ("python/nodes/orders/constants.py", ""),
                (
                    "python/nodes/orders/report.py",
                    "from python.nodes.orders.weekly.main import main\n",
                ),
                ("python/nodes/orders/_helpers/dates.py", ""),
                ("python/nodes/orders/daily/asset.py", DAILY_ASSET),
                ("python/nodes/orders/daily/_helpers/rows.py", ""),
                (
                    "python/nodes/orders/daily/main.py",
                    r#"import os
from sqlbuild import PythonNodeResult
from python.providers.store.provider import Store
from python.nodes.utils.text import clean
from python.nodes.orders.constants import LIMIT
from python.nodes.orders._helpers.dates import today
from ._helpers.rows import rows
from python.nodes.orders.report import summary
from python.nodes.orders.weekly.main import main as weekly
from python.nodes.orders.weekly.constants import WEEK
from python.nodes.orders.weekly._helpers.x import x
from python.nodes.billing.invoices.asset import invoices
from python.providers.store.client import Client


def main(ctx: AssetContext) -> PythonNodeResult:
    return invoices
"#,
                ),
                (
                    "python/nodes/orders/weekly/asset.py",
                    r#"from python.nodes.orders.daily.asset import daily
from .main import main


@asset(depends_on=[daily], description="W.")
def weekly(ctx: AssetContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/nodes/orders/weekly/main.py", ASSET_MAIN),
                ("python/nodes/orders/weekly/constants.py", ""),
                ("python/nodes/orders/weekly/_helpers/x.py", ""),
                ("python/nodes/billing/invoices/asset.py", ""),
                ("python/nodes/billing/invoices/main.py", ""),
            ],
            config: json!({ "select": ["SQBRPYTHON008"] }),
            expected_faults: vec![
                ("SQBRPYTHON008", "python/nodes/orders/daily/main.py", 8, 34),
                ("SQBRPYTHON008", "python/nodes/orders/daily/main.py", 9, 39),
                ("SQBRPYTHON008", "python/nodes/orders/daily/main.py", 10, 44),
                ("SQBRPYTHON008", "python/nodes/orders/daily/main.py", 11, 45),
                ("SQBRPYTHON008", "python/nodes/orders/daily/main.py", 12, 42),
                ("SQBRPYTHON008", "python/nodes/orders/daily/main.py", 13, 36),
            ],
        },
        PythonRunTestCase {
            description: "checks.py: statements, a check of another unit's node, a check that computes, and a plain function report",
            files: &[
                ("python/nodes/orders/daily/asset.py", DAILY_ASSET),
                (
                    "python/nodes/orders/weekly/asset.py",
                    "@asset\ndef weekly(ctx):\n    pass\n",
                ),
                (
                    "python/nodes/orders/daily/checks.py",
                    r#"from python.nodes.orders.weekly.asset import weekly

from .asset import daily

LIMIT = 3


@check(depends_on=[daily], description="A.")
def owns(ctx: CheckContext) -> bool:
    return True


@check(depends_on=[weekly], description="B.")
def foreign(ctx: CheckContext) -> bool:
    return True


@check(depends_on=[daily], description="C.")
def computes(ctx: CheckContext) -> bool:
    total = 1
    return total > 0


def helper() -> None:
    pass
"#,
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON009"] }),
            expected_faults: vec![
                ("SQBRPYTHON009", "python/nodes/orders/daily/checks.py", 5, 1),
                ("SQBRPYTHON009", "python/nodes/orders/daily/checks.py", 14, 5),
                ("SQBRPYTHON009", "python/nodes/orders/daily/checks.py", 19, 5),
                ("SQBRPYTHON009", "python/nodes/orders/daily/checks.py", 24, 5),
            ],
        },
        PythonRunTestCase {
            description: "factories: specs from outside the unit, an unnamed generated node and no loop report",
            files: &[
                ("python/nodes/orders/constants.py", "MARKETS: tuple = ()\n"),
                (
                    "python/nodes/orders/markets/assets.py",
                    r#"from python.nodes.orders.constants import MARKETS
from .main import main


@factory
def market_exports():
    for market in MARKETS:

        @asset(description="M.")
        def export(ctx: AssetContext) -> None:
            return main(ctx=ctx)
"#,
                ),
                (
                    "python/nodes/orders/once/assets.py",
                    r#"from .main import main


@factory
def once():
    @asset(name="once", description="O.")
    def run(ctx: AssetContext) -> None:
        return main(ctx=ctx)
"#,
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON010"] }),
            expected_faults: vec![
                ("SQBRPYTHON010", "python/nodes/orders/markets/assets.py", 6, 5),
                ("SQBRPYTHON010", "python/nodes/orders/markets/assets.py", 10, 13),
                ("SQBRPYTHON010", "python/nodes/orders/once/assets.py", 5, 5),
            ],
        },
        PythonRunTestCase {
            description: "descriptions: a docstring, a described loader and an undescribed check report",
            files: &[
                (
                    "python/nodes/orders/daily/asset.py",
                    r#"from .main import main


@asset(description="A.")
def described(ctx: AssetContext) -> None:
    return main(ctx=ctx)


@asset
def bare(ctx: AssetContext) -> None:
    """Docstring."""
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/nodes/orders/daily/checks.py",
                    r#"from .asset import described


@check(depends_on=[described])
def quality(ctx: CheckContext) -> bool:
    return True
"#,
                ),
                (
                    "python/nodes/orders/raw/loader.py",
                    r#"from .main import main


@loader(description="Raw.")
def raw(ctx: LoaderContext) -> list:
    return main(ctx=ctx)
"#,
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON011"] }),
            expected_faults: vec![
                ("SQBRPYTHON011", "python/nodes/orders/daily/checks.py", 5, 5),
                ("SQBRPYTHON011", "python/nodes/orders/daily/asset.py", 10, 5),
                ("SQBRPYTHON011", "python/nodes/orders/raw/loader.py", 5, 5),
            ],
        },
        PythonRunTestCase {
            description: "declarations outside a unit's asset.py or checks.py report; a factory's checks in asset.py do not",
            files: &[
                (
                    "python/nodes/orders/loaders.py",
                    r#"@loader
def raw_orders(ctx: LoaderContext) -> list:
    return []


@check(depends_on=[raw_orders], description="Q.")
def raw_orders_present(ctx: CheckContext) -> bool:
    return True
"#,
                ),
                (
                    "python/nodes/orders/daily/main.py",
                    "@asset(description=\"X.\")\ndef stray(ctx: AssetContext) -> None:\n    return None\n",
                ),
                (
                    "python/nodes/orders/daily/asset.py",
                    r#"from .constants import TABLES
from .main import main


@factory
def tables():
    for table in TABLES:

        @asset(name=table, description="T.")
        def load(ctx: AssetContext) -> None:
            return main(ctx=ctx)

        @check(name=f"{table}_ok", depends_on=[load], description="C.")
        def ok(ctx: CheckContext) -> bool:
            return True
"#,
                ),
                (
                    "python/nodes/orders/daily/checks.py",
                    "@factory\ndef audits():\n    pass\n",
                ),
                (
                    "python/nodes/orders/quality/factories.py",
                    "@audit_factory\ndef audits():\n    pass\n",
                ),
                (
                    "python/nodes/utils/flat.py",
                    "@asset\ndef util_asset(ctx):\n    return 1\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON012"] }),
            expected_faults: vec![
                ("SQBRPYTHON012", "python/nodes/orders/daily/main.py", 2, 5),
                ("SQBRPYTHON012", "python/nodes/orders/loaders.py", 2, 5),
                ("SQBRPYTHON012", "python/nodes/orders/loaders.py", 7, 5),
                ("SQBRPYTHON012", "python/nodes/utils/flat.py", 2, 5),
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

#[test]
fn given_valid_shapes_and_hidden_mistakes_when_evaluating_unit_rules_then_reports_only_the_mistakes()
 {
    install_test_identity();
    let test_cases = [
        PythonRunTestCase {
            description: "a asset.py in utils/, a role folder or a unit's _helpers/ is not a unit",
            files: &[
                ("python/nodes/utils/graph/asset.py", "class Node:\n    pass\n"),
                ("python/nodes/orders/models/asset.py", "class Node:\n    pass\n"),
                ("python/nodes/orders/daily/asset.py", DAILY_ASSET),
                ("python/nodes/orders/daily/main.py", ASSET_MAIN),
                (
                    "python/nodes/orders/daily/_helpers/asset.py",
                    "class Node:\n    pass\n",
                ),
                (
                    "python/nodes/orders/daily/_helpers/rows.py",
                    "from python.nodes.orders.daily.main import main\n",
                ),
            ],
            config: json!({ "select": UNIT_RULES }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "a factory may read specs through a method, a module attribute or a builtin call, and loop again inside",
            files: &[
                (
                    "python/nodes/orders/regions/constants.py",
                    "SPECS: dict = {}\nTABLES: tuple = ()\n",
                ),
                (
                    "python/nodes/orders/regions/factory.py",
                    r#"from . import constants
from .constants import SPECS
from .main import main


@factory
def region_exports():
    for region, spec in SPECS.items():
        for column in spec.columns:

            @asset(name=f"export_{region}_{column}", description="E.")
            def export(ctx: AssetContext) -> None:
                return main(ctx=ctx, region=region)

    for table in constants.TABLES:

        @asset(name=table, description="T.")
        def load(ctx: AssetContext) -> None:
            return main(ctx=ctx)
"#,
                ),
                ("python/nodes/orders/regions/main.py", ASSET_MAIN),
                ("python/nodes/orders/ranked/constants.py", "SPECS: dict = {}\n"),
                (
                    "python/nodes/orders/ranked/assets.py",
                    r#"from .constants import SPECS
from .main import main


@factory
def ranked_exports():
    for rank, region in enumerate(sorted(SPECS)):

        @asset(name=f"export_{rank}_{region}", description="E.")
        def export(ctx: AssetContext) -> None:
            return main(ctx=ctx, region=region)
"#,
                ),
                ("python/nodes/orders/ranked/main.py", ASSET_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON010"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "imports under if TYPE_CHECKING: are imports, and their boundaries are checked",
            files: &[
                ("python/providers/store/provider.py", ""),
                (
                    "python/nodes/orders/daily/asset.py",
                    r#"from typing import TYPE_CHECKING

from .main import main

if TYPE_CHECKING:
    from python.providers.store.provider import Store
    from python.nodes.orders.weekly.main import main as weekly_main


@asset(description="Daily.")
def daily(ctx: AssetContext, *, store: "Store") -> None:
    return main(ctx=ctx, store=store)
"#,
                ),
                ("python/nodes/orders/daily/main.py", ASSET_MAIN),
                ("python/nodes/orders/weekly/asset.py", DAILY_ASSET),
                ("python/nodes/orders/weekly/main.py", ASSET_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON004", "SQBRPYTHON007", "SQBRPYTHON008"] }),
            expected_faults: vec![("SQBRPYTHON008", "python/nodes/orders/daily/asset.py", 7, 49)],
        },
        PythonRunTestCase {
            description: "a node named through import a.b in depends_on passes; an unused import of a node does not",
            files: &[
                ("python/nodes/orders/daily/asset.py", DAILY_ASSET),
                ("python/nodes/orders/daily/main.py", ASSET_MAIN),
                (
                    "python/nodes/orders/weekly/asset.py",
                    r#"import python.nodes.orders.daily.node

from .main import main


@asset(depends_on=[python.nodes.orders.daily.asset.daily], description="W.")
def weekly(ctx: AssetContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/nodes/orders/weekly/main.py", ASSET_MAIN),
                (
                    "python/nodes/orders/weekly/_helpers/rows.py",
                    "from python.nodes.orders.daily.asset import daily\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON008"] }),
            expected_faults: vec![(
                "SQBRPYTHON008",
                "python/nodes/orders/weekly/_helpers/rows.py",
                1,
                38,
            )],
        },
        PythonRunTestCase {
            description: "a quoted union result and a loader's tuple of rows fit their node",
            files: &[
                ("python/nodes/orders/daily/asset.py", DAILY_ASSET),
                (
                    "python/nodes/orders/daily/main.py",
                    "def main(ctx: AssetContext) -> \"PythonNodeResult | PythonNodeSkipResult\":\n    return ctx.result()\n",
                ),
                (
                    "python/nodes/orders/raw/loader.py",
                    "from .main import main\n\n\n@loader\ndef raw(ctx: LoaderContext) -> tuple:\n    return main(ctx=ctx)\n",
                ),
                (
                    "python/nodes/orders/raw/main.py",
                    "def main(ctx: LoaderContext) -> \"tuple[python.nodes.orders.raw.types.Row, ...]\":\n    return ()\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON006"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "a node that calls the main.py module instead of main() reports",
            files: &[
                (
                    "python/nodes/orders/daily/asset.py",
                    "from . import main\n\n\n@asset(description=\"D.\")\ndef daily(ctx: AssetContext) -> None:\n    return main(ctx=ctx)\n",
                ),
                ("python/nodes/orders/daily/main.py", ASSET_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON005"] }),
            expected_faults: vec![("SQBRPYTHON005", "python/nodes/orders/daily/asset.py", 5, 5)],
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
