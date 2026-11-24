"""Functions, reference sources, audit factories and the few hand-shaped models around them."""

from __future__ import annotations

from duckshop10k.core import Column, Model, Project, cte, import_cte, select_list
from duckshop10k.layers import Intermediate
from duckshop10k.staging import with_query
from duckshop10k.world import CURRENCY_EXPONENT

SKU_FAMILY = """FUNCTION (
  description "Product family prefix of a canonical SKU, such as DECOY for DECOY-TEAL-08",
  arguments (sku VARCHAR),
  returns VARCHAR,
);

UPPER(REGEXP_SUBSTR(sku, '^[A-Za-z]+'))
"""

LUHN_VALID = '''from sqlbuild.functions import udf


@udf(
    arguments={"digits": "VARCHAR"},
    returns="BOOLEAN",
    runtime_version="3.11",
)
def main(digits: str | None) -> bool | None:
    """Whether a string of digits passes the Luhn checksum; NULL for empty or non-numeric input."""
    if not digits or not digits.isdigit():
        return None
    total = 0
    for index, char in enumerate(reversed(digits)):
        value = int(char)
        if index % 2 == 1:
            value *= 2
            if value > 9:
                value -= 9
        total += value
    return total % 10 == 0
'''

CUSTOMER_ORDERS = """FUNCTION (
  description "Orders placed by one storefront customer with their USD value and status",
  arguments (p_customer_id VARCHAR),
  returns table (
    order_id VARCHAR,
    storefront_code VARCHAR,
    placed_at_utc TIMESTAMP_NTZ,
    status VARCHAR,
    gross_amount_usd "NUMBER(18, 2)"
  )
);

WITH orders AS (
  SELECT *
  FROM __ref("fct_orders")
),

final AS (
  SELECT
    o.order_id,
    o.storefront_code,
    o.placed_at_utc,
    o.status,
    o.gross_amount_usd
  FROM orders AS o
  WHERE o.customer_id = p_customer_id
)

SELECT *
FROM final
"""

FACT_QUALITY = '''"""Audit cases shared by every published fact table."""

from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory


@audit_factory
def fact_table_quality() -> list[AuditCase]:
    """A fact table must never publish empty."""
    return [
        AuditCase(
            name="fact_table_has_rows",
            definition="has_rows",
            arguments={"minimum_rows": 1},
            severity=AuditSeverity.WARN,
            description="A fact table built with no rows usually means an upstream load failed.",
            meta={"check": "volume"},
        )
    ]
'''

# Reference rates to USD, invented but plausible, on three dates.
_RATES: dict[str, tuple[float, float, float]] = {
    "AED": (0.27230, 0.27230, 0.27230),
    "AUD": (0.61900, 0.65800, 0.66250),
    "BHD": (2.65250, 2.65250, 2.65250),
    "BRL": (0.16200, 0.17100, 0.16950),
    "CAD": (0.69500, 0.73200, 0.73700),
    "CHF": (1.10300, 1.12800, 1.13100),
    "CLP": (0.00101, 0.00106, 0.00105),
    "COP": (0.00023, 0.00025, 0.00025),
    "CZK": (0.04120, 0.04310, 0.04290),
    "DKK": (0.13870, 0.14510, 0.14620),
    "EUR": (1.03500, 1.08250, 1.09100),
    "GBP": (1.25100, 1.26400, 1.27150),
    "HUF": (0.00252, 0.00271, 0.00274),
    "INR": (0.01168, 0.01202, 0.01198),
    "JPY": (0.00636, 0.00682, 0.00672),
    "KRW": (0.00068, 0.00073, 0.00072),
    "KWD": (3.24300, 3.25400, 3.25150),
    "MXN": (0.04910, 0.05720, 0.05660),
    "NOK": (0.08810, 0.09340, 0.09410),
    "NZD": (0.56100, 0.60200, 0.60600),
    "PLN": (0.24300, 0.25100, 0.25350),
    "QAR": (0.27470, 0.27470, 0.27470),
    "SAR": (0.26650, 0.26650, 0.26650),
    "SEK": (0.09040, 0.09560, 0.09710),
    "SGD": (0.73300, 0.74500, 0.74800),
    "THB": (0.02920, 0.02810, 0.02840),
    "TWD": (0.03050, 0.03120, 0.03100),
    "USD": (1.00000, 1.00000, 1.00000),
    "ZAR": (0.05320, 0.05510, 0.05470),
}
_RATE_DATES = ("2025-01-01", "2026-01-01", "2026-03-01")


def fx_source() -> str:
    assert set(_RATES) == set(CURRENCY_EXPONENT)
    rows = [
        f"          ('{code}', '{day}', {rates[index]:.8f})"
        for code, rates in sorted(_RATES.items())
        for index, day in enumerate(_RATE_DATES)
    ]
    return (
        "# Reference data defined inline so the project compiles without an FX feed.\n"
        "sources:\n"
        "  - name: finance__fx_rates\n"
        "    description: Daily reference rates to USD per currency, defined inline as VALUES\n"
        "    contract: enforced\n"
        "    expression: |\n"
        "      SELECT v.currency_code, v.rate_date, v.rate_to_usd\n"
        "      FROM (\n"
        "        VALUES\n"
        + ",\n".join(rows)
        + "\n      ) AS v (currency_code, rate_date, rate_to_usd)\n"
        "    columns:\n"
        "      - name: currency_code\n"
        "        type: VARCHAR\n"
        "        nullable: false\n"
        "        audits:\n"
        "          - not_null\n"
        "      - name: rate_date\n"
        "        type: DATE\n"
        "        nullable: false\n"
        "        audits:\n"
        "          - not_null\n"
        "      - name: rate_to_usd\n"
        "        type: NUMBER(18,8)\n"
        "        nullable: false\n"
    )


def build_extras(project: Project, result: Intermediate) -> None:
    project.write("sql/functions/sql/udf__sku_family.sql", SKU_FAMILY)
    project.write("sql/functions/python/luhn_valid.py", LUHN_VALID)
    project.write("python/nodes/audits/fact_quality.py", FACT_QUALITY)
    project.write("sql/sources/finance_reference.yml", fx_source())
    charges = result.combined.get("payments__charges_enriched")
    if charges is not None:
        _card_checks(project, charges)
    if "fct_orders" in project.models:
        project.write("sql/functions/sql/table_fn__customer_orders.sql", CUSTOMER_ORDERS)
        _order_lookup(project)
    tickets = result.combined.get("support__tickets_enriched")
    if tickets is not None:
        _open_ticket_age(project, tickets)
    orders = result.combined.get("suppliers__purchase_orders_enriched")
    if orders is not None:
        _overdue_purchase_orders(project, orders)
    _dim_date(project)


def _card_checks(project: Project, charges: Model) -> None:
    items = [
        "c.charge_id",
        "c.gateway_code",
        "c.card_bin",
        '__udf("luhn_valid")(c.card_bin)::BOOLEAN AS is_card_bin_checksum_valid',
        "REGEXP_LIKE(c.card_bin, '^[0-9]{6,8}$')::BOOLEAN AS is_card_bin_well_formed",
        "c.charged_amount",
    ]
    columns = [
        Column("charge_id", "VARCHAR", ("not_null", "unique"), nullable=False),
        Column("gateway_code", "VARCHAR"),
        Column("card_bin", "VARCHAR"),
        Column("is_card_bin_checksum_valid", "BOOLEAN"),
        Column("is_card_bin_well_formed", "BOOLEAN"),
        Column("charged_amount", "NUMBER(18, 2)"),
    ]
    project.add(
        Model(
            name="int_payments__card_bin_checks",
            folder="sql/models/intermediate/payments",
            layer="intermediate",
            description="One row per gateway charge with checks on the card number"
            " prefix it reported",
            columns=columns,
            body=with_query(
                [
                    import_cte("charges", f'__ref("{charges.name}")'),
                    cte(
                        "final",
                        f"SELECT\n{select_list(items)}\nFROM charges AS c\nWHERE\n "
                        " c.card_bin IS NOT NULL",
                    ),
                ]
            ),
            refs=[charges.name],
        )
    )


def _order_lookup(project: Project) -> None:
    project.add(
        Model(
            name="rpt_customer_order_lookup",
            folder="sql/models/marts/support",
            layer="marts",
            description=(
                "Every order of the customer named by the lookup_customer_id variable, for "
                "support agents, read through the customer orders table function"
            ),
            columns=[
                Column("order_id", "VARCHAR", ("not_null", "unique"), nullable=False),
                Column("storefront_code", "VARCHAR"),
                Column("placed_at_utc", "TIMESTAMP_NTZ"),
                Column("status", "VARCHAR"),
                Column("gross_amount_usd", "NUMBER(18, 2)"),
            ],
            body=with_query(
                [
                    cte(
                        "customer_orders",
                        'SELECT *\nFROM __table_fn("table_fn__customer_orders")'
                        "('@@lookup_customer_id')",
                    ),
                    cte(
                        "final",
                        "SELECT\n"
                        + select_list(
                            [
                                "o.order_id",
                                "o.storefront_code",
                                "o.placed_at_utc",
                                "o.status",
                                "o.gross_amount_usd",
                            ]
                        )
                        + "\nFROM customer_orders AS o",
                    ),
                ]
            ),
        )
    )


def _open_ticket_age(project: Project, tickets: Model) -> None:
    items = [
        "t.ticket_id",
        "t.support_tool_code",
        "t.contact_channel",
        "t.created_at_utc",
        "DATEDIFF(\n    'hour',\n    t.created_at_utc,\n"
        "    CONVERT_TIMEZONE('UTC', CURRENT_TIMESTAMP())::TIMESTAMP_NTZ\n  )::INT AS age_hours",
    ]
    project.add(
        Model(
            name="rpt_support__open_ticket_age",
            folder="sql/models/marts/support",
            layer="marts",
            description=(
                "Tickets with no satisfaction response yet and their age in hours at query time. "
                "The age reads CURRENT_TIMESTAMP(), which is session-dependent, so it is an "
                "expected divergence across engines."
            ),
            tags=["known_divergence"],
            columns=[
                Column("ticket_id", "VARCHAR", ("not_null", "unique"), nullable=False),
                Column("support_tool_code", "VARCHAR"),
                Column("contact_channel", "VARCHAR"),
                Column("created_at_utc", "TIMESTAMP_NTZ"),
                Column("age_hours", "INT"),
            ],
            body=with_query(
                [
                    import_cte("tickets", f'__ref("{tickets.name}")'),
                    cte(
                        "final",
                        f"SELECT\n{select_list(items)}\nFROM tickets AS t\nWHERE\n"
                        "  t.csat_responses_count = 0",
                    ),
                ]
            ),
            refs=[tickets.name],
        )
    )


def _overdue_purchase_orders(project: Project, orders: Model) -> None:
    items = [
        "p.po_id",
        "p.supplier_code",
        "p.warehouse_code",
        "p.expected_at_utc",
        "DATEDIFF(\n    'day',\n    p.expected_at_utc,\n"
        "    CONVERT_TIMEZONE('UTC', CURRENT_TIMESTAMP())::TIMESTAMP_NTZ\n  )::INT AS days_overdue",
    ]
    project.add(
        Model(
            name="rpt_operations__overdue_purchase_orders",
            folder="sql/models/marts/operations",
            layer="marts",
            description=(
                "Purchase orders past their expected arrival with no invoice yet, aged at query "
                "time from CURRENT_TIMESTAMP(), which is session-dependent and an expected "
                "divergence across engines"
            ),
            tags=["known_divergence"],
            columns=[
                Column("po_id", "VARCHAR", ("not_null", "unique"), nullable=False),
                Column("supplier_code", "VARCHAR"),
                Column("warehouse_code", "VARCHAR"),
                Column("expected_at_utc", "TIMESTAMP_NTZ"),
                Column("days_overdue", "INT"),
            ],
            body=with_query(
                [
                    import_cte("purchase_orders", f'__ref("{orders.name}")'),
                    cte(
                        "final",
                        f"SELECT\n{select_list(items)}\nFROM purchase_orders AS p\nWHERE\n"
                        "  p.invoices_count = 0\n"
                        "  AND p.expected_at_utc < CONVERT_TIMEZONE('UTC',"
                        " CURRENT_TIMESTAMP())::TIMESTAMP_NTZ",
                    ),
                ]
            ),
            refs=[orders.name],
        )
    )


def _dim_date(project: Project) -> None:
    items = [
        "s.calendar_date::DATE AS calendar_date",
        "YEAR(s.calendar_date)::INT AS calendar_year",
        "MONTH(s.calendar_date)::INT AS calendar_month",
        "DAYOFWEEKISO(s.calendar_date)::INT AS iso_day_of_week",
        "@sql.week_start(s.calendar_date) AS week_start_date",
        "(DAYOFWEEKISO(s.calendar_date) >= 6)::BOOLEAN AS is_weekend",
    ]
    project.add(
        Model(
            name="dim_date",
            folder="sql/models/marts/core",
            layer="marts",
            description="One row per calendar day of the reporting history, from GENERATOR",
            config=["contract enforced"],
            columns=[
                Column("calendar_date", "DATE", ("not_null", "unique"), nullable=False),
                Column("calendar_year", "INT"),
                Column("calendar_month", "INT"),
                Column("iso_day_of_week", "INT"),
                Column("week_start_date", "DATE"),
                Column("is_weekend", "BOOLEAN"),
            ],
            body=with_query(
                [
                    cte(
                        "days",
                        "SELECT SEQ4() AS day_offset\nFROM TABLE(GENERATOR(ROWCOUNT => 1095))",
                    ),
                    cte(
                        "spine",
                        "SELECT\n  DATEADD('day', d.day_offset, "
                        '@const("history_start_date")::DATE)'
                        "::DATE AS calendar_date\nFROM days AS d",
                    ),
                    cte("final", f"SELECT\n{select_list(items)}\nFROM spine AS s"),
                ]
            ),
        )
    )
