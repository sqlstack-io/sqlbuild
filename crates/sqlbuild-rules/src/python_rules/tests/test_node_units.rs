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

const DAILY_TASK: &str = r#"from .main import main


@task(description="Daily.")
def daily(ctx: TaskContext) -> None:
    return main(ctx=ctx)
"#;

const TASK_MAIN: &str = r#"from sqlbuild import PythonNodeResult


def main(ctx: TaskContext) -> PythonNodeResult:
    return ctx.result()
"#;

#[test]
fn given_node_units_when_evaluating_unit_rules_then_reports_each_broken_convention() {
    let test_cases = [
        PythonRunTestCase {
            description: "a project laid out in units reports nothing",
            files: &[
                ("providers/store/provider.py", "class Store:\n    pass\n"),
                (
                    "python/utils/text.py",
                    "def clean() -> str:\n    return ''\n",
                ),
                ("python/orders/constants.py", "LIMIT: int = 3\n"),
                (
                    "python/orders/_helpers/dates.py",
                    "def today() -> str:\n    return ''\n",
                ),
                (
                    "python/orders/daily/node.py",
                    r#"from providers.store.provider import Store

from .main import main


@task(description="Daily.")
def daily(ctx: TaskContext, *, store: Store) -> None:
    return main(ctx=ctx, store=store)
"#,
                ),
                (
                    "python/orders/daily/main.py",
                    r#""""Daily totals."""
from sqlbuild import PythonNodeResult, PythonNodeSkipResult

from providers.store.provider import Store
from python.orders._helpers.dates import today
from python.orders.constants import LIMIT
from python.utils.text import clean

from ._helpers.rows import rows


def main(ctx: TaskContext, store: Store) -> PythonNodeResult | PythonNodeSkipResult:
    return ctx.result(rows(today(), clean(), LIMIT, store))
"#,
                ),
                (
                    "python/orders/daily/_helpers/rows.py",
                    "def rows() -> list:\n    return []\n",
                ),
                (
                    "python/orders/daily/checks.py",
                    r#"from .node import daily


@check(depends_on=[daily], description="Totals are positive.")
def totals_positive(ctx: CheckContext) -> bool:
    return ctx.ok()
"#,
                ),
                (
                    "python/orders/weekly/node.py",
                    r#"from python.orders.daily.node import daily
from python.orders.weekly.main import main


@asset(depends_on=[daily], description="Weekly.")
def weekly(ctx: AssetContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/orders/weekly/main.py",
                    "def main(ctx: AssetContext) -> PythonNodeResult:\n    return ctx.result()\n",
                ),
                (
                    "python/orders/raw/node.py",
                    r#"from .main import main


@loader
def raw(ctx: LoaderContext) -> list:
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/orders/raw/main.py",
                    "def main(ctx: LoaderContext) -> Optional[Iterator[dict]]:\n    return None\n",
                ),
                (
                    "python/orders/regions/node.py",
                    r#"from .constants import REGIONS
from .main import main


@factory
def region_exports():
    for region in REGIONS:

        @task(name=f"export_{region}", description="Export one region.")
        def export(ctx: TaskContext) -> None:
            return main(ctx=ctx, region=region)

        @check(name=f"export_{region}_ok", depends_on=[export], description="Exported.")
        def exported(ctx: CheckContext) -> bool:
            return True
"#,
                ),
                (
                    "python/orders/regions/constants.py",
                    "REGIONS: tuple = ('eu', 'us')\n",
                ),
                (
                    "python/orders/regions/main.py",
                    "def main(ctx: TaskContext, region: str) -> PythonNodeResult:\n    return ctx.result()\n",
                ),
            ],
            config: json!({ "select": UNIT_RULES }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "unit layout: a missing main.py, a stray module and a folder that is not _helpers/",
            files: &[
                ("python/orders/daily/node.py", DAILY_TASK),
                ("python/orders/weekly/node.py", ""),
                ("python/orders/weekly/main.py", ""),
                ("python/orders/weekly/report.py", ""),
                ("python/orders/weekly/sql/query.py", ""),
                ("python/orders/weekly/_helpers/rows.py", ""),
            ],
            config: json!({ "select": ["SQBRPYTHON003"] }),
            expected_faults: vec![
                ("SQBRPYTHON003", "python/orders/daily/node.py", 1, 1),
                ("SQBRPYTHON003", "python/orders/weekly/report.py", 1, 1),
                ("SQBRPYTHON003", "python/orders/weekly/sql/query.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "node.py: statements, helpers and a second declaration report; an empty node.py reports at its start",
            files: &[
                (
                    "python/orders/daily/node.py",
                    r#""""Daily."""
from .main import main

LIMIT = 3


def helper() -> int:
    return 1


@task(description="Daily.")
def daily(ctx: TaskContext) -> None:
    return main(ctx=ctx)


@task(description="Extra.")
def extra(ctx: TaskContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/orders/weekly/node.py", "from .main import main\n"),
            ],
            config: json!({ "select": ["SQBRPYTHON004"] }),
            expected_faults: vec![
                ("SQBRPYTHON004", "python/orders/daily/node.py", 4, 1),
                ("SQBRPYTHON004", "python/orders/daily/node.py", 7, 5),
                ("SQBRPYTHON004", "python/orders/daily/node.py", 17, 5),
                ("SQBRPYTHON004", "python/orders/weekly/node.py", 1, 1),
            ],
        },
        PythonRunTestCase {
            description: "delegation: positional arguments, extra statements, unpacking and another unit's main report",
            files: &[
                (
                    "python/orders/daily/node.py",
                    r#"from python.orders.daily.main import main
from python.orders.weekly.main import main as weekly_main


@task(description="A.")
def by_keyword(ctx: TaskContext) -> None:
    return main(ctx=ctx)


@task(description="B.")
def positional(ctx: TaskContext) -> None:
    return main(ctx)


@task(description="C.")
def computes(ctx: TaskContext) -> None:
    rows = main(ctx=ctx)
    return rows


@task(description="D.")
def unpacked(ctx: TaskContext) -> None:
    return main(**ctx)
"#,
                ),
                ("python/orders/daily/main.py", TASK_MAIN),
                (
                    "python/orders/weekly/node.py",
                    r#"from python.orders.daily.main import main


@task(description="W.")
def weekly(ctx: TaskContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/orders/weekly/main.py", TASK_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON005"] }),
            expected_faults: vec![
                ("SQBRPYTHON005", "python/orders/daily/node.py", 11, 5),
                ("SQBRPYTHON005", "python/orders/daily/node.py", 16, 5),
                ("SQBRPYTHON005", "python/orders/daily/node.py", 22, 5),
                ("SQBRPYTHON005", "python/orders/weekly/node.py", 5, 5),
            ],
        },
        PythonRunTestCase {
            description: "main.py: other statements, a result that does not fit the node kind, and no main()",
            files: &[
                ("python/orders/daily/node.py", DAILY_TASK),
                (
                    "python/orders/daily/main.py",
                    r#"from sqlbuild import PythonNodeResult

LIMIT = 3


def main(ctx: TaskContext) -> list:
    return []


def helper() -> None:
    pass
"#,
                ),
                ("python/orders/weekly/node.py", ""),
                (
                    "python/orders/weekly/main.py",
                    "def run() -> None:\n    pass\n",
                ),
                ("python/orders/stock/node.py", ""),
                ("python/orders/stock/main.py", "def main(ctx):\n    pass\n"),
                (
                    "python/orders/raw/node.py",
                    "from .main import main\n\n\n@loader\ndef raw(ctx: LoaderContext) -> list:\n    return main(ctx=ctx)\n",
                ),
                (
                    "python/orders/raw/main.py",
                    "def main(ctx: LoaderContext) -> PythonNodeResult:\n    return []\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON006"] }),
            expected_faults: vec![
                ("SQBRPYTHON006", "python/orders/daily/main.py", 3, 1),
                ("SQBRPYTHON006", "python/orders/daily/main.py", 6, 5),
                ("SQBRPYTHON006", "python/orders/daily/main.py", 10, 5),
                ("SQBRPYTHON006", "python/orders/raw/main.py", 1, 5),
                ("SQBRPYTHON006", "python/orders/stock/main.py", 1, 5),
                ("SQBRPYTHON006", "python/orders/weekly/main.py", 1, 1),
                ("SQBRPYTHON006", "python/orders/weekly/main.py", 1, 5),
            ],
        },
        PythonRunTestCase {
            description: "signature: ctx missing or mistyped, variadics and unannotated providers report; positional-only ctx does not",
            files: &[
                (
                    "python/orders/daily/node.py",
                    r#"from .main import main


@task(description="A.")
def no_ctx(context: TaskContext) -> None:
    return main(ctx=context)


@task(description="B.")
def wrong_type(ctx: AssetContext, *, store: Store) -> None:
    return main(ctx=ctx, store=store)


@task(description="C.")
def loose(ctx: TaskContext, *args, store, **kwargs) -> None:
    return main(ctx=ctx)


@task(description="D.")
def positional_only(ctx: TaskContext, /) -> None:
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/orders/daily/checks.py",
                    r#"from .node import no_ctx


@check(depends_on=[no_ctx], description="Q.")
def quality(ctx: TaskContext) -> bool:
    return True
"#,
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON007"] }),
            expected_faults: vec![
                ("SQBRPYTHON007", "python/orders/daily/checks.py", 5, 5),
                ("SQBRPYTHON007", "python/orders/daily/node.py", 5, 5),
                ("SQBRPYTHON007", "python/orders/daily/node.py", 10, 5),
                ("SQBRPYTHON007", "python/orders/daily/node.py", 15, 5),
                ("SQBRPYTHON007", "python/orders/daily/node.py", 15, 5),
            ],
        },
        PythonRunTestCase {
            description: "imports: another unit's main, role files and helpers, a domain module, and a node used outside depends_on report",
            files: &[
                ("providers/store/provider.py", ""),
                ("providers/store/client.py", ""),
                ("python/utils/text.py", ""),
                ("python/orders/constants.py", ""),
                (
                    "python/orders/report.py",
                    "from python.orders.weekly.main import main\n",
                ),
                ("python/orders/_helpers/dates.py", ""),
                ("python/orders/daily/node.py", DAILY_TASK),
                ("python/orders/daily/_helpers/rows.py", ""),
                (
                    "python/orders/daily/main.py",
                    r#"import os
from sqlbuild import PythonNodeResult
from providers.store.provider import Store
from python.utils.text import clean
from python.orders.constants import LIMIT
from python.orders._helpers.dates import today
from ._helpers.rows import rows
from python.orders.report import summary
from python.orders.weekly.main import main as weekly
from python.orders.weekly.constants import WEEK
from python.orders.weekly._helpers.x import x
from python.billing.invoices.node import invoices
from providers.store.client import Client


def main(ctx: TaskContext) -> PythonNodeResult:
    return invoices
"#,
                ),
                (
                    "python/orders/weekly/node.py",
                    r#"from python.orders.daily.node import daily
from .main import main


@task(depends_on=[daily], description="W.")
def weekly(ctx: TaskContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/orders/weekly/main.py", TASK_MAIN),
                ("python/orders/weekly/constants.py", ""),
                ("python/orders/weekly/_helpers/x.py", ""),
                ("python/billing/invoices/node.py", ""),
                ("python/billing/invoices/main.py", ""),
            ],
            config: json!({ "select": ["SQBRPYTHON008"] }),
            expected_faults: vec![
                ("SQBRPYTHON008", "python/orders/daily/main.py", 8, 34),
                ("SQBRPYTHON008", "python/orders/daily/main.py", 9, 39),
                ("SQBRPYTHON008", "python/orders/daily/main.py", 10, 44),
                ("SQBRPYTHON008", "python/orders/daily/main.py", 11, 45),
                ("SQBRPYTHON008", "python/orders/daily/main.py", 12, 42),
                ("SQBRPYTHON008", "python/orders/daily/main.py", 13, 36),
            ],
        },
        PythonRunTestCase {
            description: "checks.py: statements, a check of another unit's node, a check that computes, and a plain function report",
            files: &[
                ("python/orders/daily/node.py", DAILY_TASK),
                (
                    "python/orders/weekly/node.py",
                    "@task\ndef weekly(ctx):\n    pass\n",
                ),
                (
                    "python/orders/daily/checks.py",
                    r#"from python.orders.weekly.node import weekly

from .node import daily

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
                ("SQBRPYTHON009", "python/orders/daily/checks.py", 5, 1),
                ("SQBRPYTHON009", "python/orders/daily/checks.py", 14, 5),
                ("SQBRPYTHON009", "python/orders/daily/checks.py", 19, 5),
                ("SQBRPYTHON009", "python/orders/daily/checks.py", 24, 5),
            ],
        },
        PythonRunTestCase {
            description: "factories: specs from outside the unit, an unnamed generated node and no loop report",
            files: &[
                ("python/orders/constants.py", "MARKETS: tuple = ()\n"),
                (
                    "python/orders/markets/node.py",
                    r#"from python.orders.constants import MARKETS
from .main import main


@factory
def market_exports():
    for market in MARKETS:

        @task(description="M.")
        def export(ctx: TaskContext) -> None:
            return main(ctx=ctx)
"#,
                ),
                (
                    "python/orders/once/node.py",
                    r#"from .main import main


@factory
def once():
    @task(name="once", description="O.")
    def run(ctx: TaskContext) -> None:
        return main(ctx=ctx)
"#,
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON010"] }),
            expected_faults: vec![
                ("SQBRPYTHON010", "python/orders/markets/node.py", 6, 5),
                ("SQBRPYTHON010", "python/orders/markets/node.py", 10, 13),
                ("SQBRPYTHON010", "python/orders/once/node.py", 5, 5),
            ],
        },
        PythonRunTestCase {
            description: "descriptions: a docstring, a described loader and an undescribed check report",
            files: &[
                (
                    "python/orders/daily/node.py",
                    r#"from .main import main


@task(description="A.")
def described(ctx: TaskContext) -> None:
    return main(ctx=ctx)


@asset
def bare(ctx: AssetContext) -> None:
    """Docstring."""
    return main(ctx=ctx)
"#,
                ),
                (
                    "python/orders/daily/checks.py",
                    r#"from .node import described


@check(depends_on=[described])
def quality(ctx: CheckContext) -> bool:
    return True
"#,
                ),
                (
                    "python/orders/raw/node.py",
                    r#"from .main import main


@loader(description="Raw.")
def raw(ctx: LoaderContext) -> list:
    return main(ctx=ctx)
"#,
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON011"] }),
            expected_faults: vec![
                ("SQBRPYTHON011", "python/orders/daily/checks.py", 5, 5),
                ("SQBRPYTHON011", "python/orders/daily/node.py", 10, 5),
                ("SQBRPYTHON011", "python/orders/raw/node.py", 5, 5),
            ],
        },
        PythonRunTestCase {
            description: "declarations outside a unit's node.py or checks.py report; a factory's checks in node.py do not",
            files: &[
                (
                    "python/orders/loaders.py",
                    r#"@loader
def raw_orders(ctx: LoaderContext) -> list:
    return []


@check(depends_on=[raw_orders], description="Q.")
def raw_orders_present(ctx: CheckContext) -> bool:
    return True
"#,
                ),
                (
                    "python/orders/daily/main.py",
                    "@task(description=\"X.\")\ndef stray(ctx: TaskContext) -> None:\n    return None\n",
                ),
                (
                    "python/orders/daily/node.py",
                    r#"from .constants import TABLES
from .main import main


@factory
def tables():
    for table in TABLES:

        @task(name=table, description="T.")
        def load(ctx: TaskContext) -> None:
            return main(ctx=ctx)

        @check(name=f"{table}_ok", depends_on=[load], description="C.")
        def ok(ctx: CheckContext) -> bool:
            return True
"#,
                ),
                (
                    "python/orders/daily/checks.py",
                    "@factory\ndef audits():\n    pass\n",
                ),
                (
                    "python/orders/quality/factories.py",
                    "@audit_factory\ndef audits():\n    pass\n",
                ),
                (
                    "python/utils/flat.py",
                    "@task\ndef util_task(ctx):\n    return 1\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON012"] }),
            expected_faults: vec![
                ("SQBRPYTHON012", "python/orders/daily/main.py", 2, 5),
                ("SQBRPYTHON012", "python/orders/loaders.py", 2, 5),
                ("SQBRPYTHON012", "python/orders/loaders.py", 7, 5),
                ("SQBRPYTHON012", "python/utils/flat.py", 2, 5),
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
    let test_cases = [
        PythonRunTestCase {
            description: "a node.py in utils/, a role folder or a unit's _helpers/ is not a unit",
            files: &[
                ("python/utils/graph/node.py", "class Node:\n    pass\n"),
                ("python/orders/models/node.py", "class Node:\n    pass\n"),
                ("python/orders/daily/node.py", DAILY_TASK),
                ("python/orders/daily/main.py", TASK_MAIN),
                (
                    "python/orders/daily/_helpers/node.py",
                    "class Node:\n    pass\n",
                ),
                (
                    "python/orders/daily/_helpers/rows.py",
                    "from python.orders.daily.main import main\n",
                ),
            ],
            config: json!({ "select": UNIT_RULES }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "a factory may read specs through a method, a module attribute or a builtin call, and loop again inside",
            files: &[
                (
                    "python/orders/regions/constants.py",
                    "SPECS: dict = {}\nTABLES: tuple = ()\n",
                ),
                (
                    "python/orders/regions/node.py",
                    r#"from . import constants
from .constants import SPECS
from .main import main


@factory
def region_exports():
    for region, spec in SPECS.items():
        for column in spec.columns:

            @task(name=f"export_{region}_{column}", description="E.")
            def export(ctx: TaskContext) -> None:
                return main(ctx=ctx, region=region)

    for table in constants.TABLES:

        @task(name=table, description="T.")
        def load(ctx: TaskContext) -> None:
            return main(ctx=ctx)
"#,
                ),
                ("python/orders/regions/main.py", TASK_MAIN),
                ("python/orders/ranked/constants.py", "SPECS: dict = {}\n"),
                (
                    "python/orders/ranked/node.py",
                    r#"from .constants import SPECS
from .main import main


@factory
def ranked_exports():
    for rank, region in enumerate(sorted(SPECS)):

        @task(name=f"export_{rank}_{region}", description="E.")
        def export(ctx: TaskContext) -> None:
            return main(ctx=ctx, region=region)
"#,
                ),
                ("python/orders/ranked/main.py", TASK_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON010"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "imports under if TYPE_CHECKING: are imports, and their boundaries are checked",
            files: &[
                ("providers/store/provider.py", ""),
                (
                    "python/orders/daily/node.py",
                    r#"from typing import TYPE_CHECKING

from .main import main

if TYPE_CHECKING:
    from providers.store.provider import Store
    from python.orders.weekly.main import main as weekly_main


@task(description="Daily.")
def daily(ctx: TaskContext, *, store: "Store") -> None:
    return main(ctx=ctx, store=store)
"#,
                ),
                ("python/orders/daily/main.py", TASK_MAIN),
                ("python/orders/weekly/node.py", DAILY_TASK),
                ("python/orders/weekly/main.py", TASK_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON004", "SQBRPYTHON007", "SQBRPYTHON008"] }),
            expected_faults: vec![("SQBRPYTHON008", "python/orders/daily/node.py", 7, 43)],
        },
        PythonRunTestCase {
            description: "a node named through import a.b in depends_on passes; an unused import of a node does not",
            files: &[
                ("python/orders/daily/node.py", DAILY_TASK),
                ("python/orders/daily/main.py", TASK_MAIN),
                (
                    "python/orders/weekly/node.py",
                    r#"import python.orders.daily.node

from .main import main


@task(depends_on=[python.orders.daily.node.daily], description="W.")
def weekly(ctx: TaskContext) -> None:
    return main(ctx=ctx)
"#,
                ),
                ("python/orders/weekly/main.py", TASK_MAIN),
                (
                    "python/orders/weekly/_helpers/rows.py",
                    "from python.orders.daily.node import daily\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON008"] }),
            expected_faults: vec![(
                "SQBRPYTHON008",
                "python/orders/weekly/_helpers/rows.py",
                1,
                38,
            )],
        },
        PythonRunTestCase {
            description: "a quoted union result and a loader's tuple of rows fit their node",
            files: &[
                ("python/orders/daily/node.py", DAILY_TASK),
                (
                    "python/orders/daily/main.py",
                    "def main(ctx: TaskContext) -> \"PythonNodeResult | PythonNodeSkipResult\":\n    return ctx.result()\n",
                ),
                (
                    "python/orders/raw/node.py",
                    "from .main import main\n\n\n@loader\ndef raw(ctx: LoaderContext) -> tuple:\n    return main(ctx=ctx)\n",
                ),
                (
                    "python/orders/raw/main.py",
                    "def main(ctx: LoaderContext) -> \"tuple[python.orders.raw.types.Row, ...]\":\n    return ()\n",
                ),
            ],
            config: json!({ "select": ["SQBRPYTHON006"] }),
            expected_faults: vec![],
        },
        PythonRunTestCase {
            description: "a node that calls the main.py module instead of main() reports",
            files: &[
                (
                    "python/orders/daily/node.py",
                    "from . import main\n\n\n@task(description=\"D.\")\ndef daily(ctx: TaskContext) -> None:\n    return main(ctx=ctx)\n",
                ),
                ("python/orders/daily/main.py", TASK_MAIN),
            ],
            config: json!({ "select": ["SQBRPYTHON005"] }),
            expected_faults: vec![("SQBRPYTHON005", "python/orders/daily/node.py", 5, 5)],
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
