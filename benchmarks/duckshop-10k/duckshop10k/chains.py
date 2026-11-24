"""The long chains: customer value and segments, the finance close, scorecards and board packs."""

from __future__ import annotations

from duckshop10k import marts
from duckshop10k.core import Column, Model, Project, cte, import_cte, select_list
from duckshop10k.intermediate import to_usd
from duckshop10k.layers import Intermediate
from duckshop10k.staging import with_query
from duckshop10k.world import REGIONS


def _model(
    project: Project,
    name: str,
    folder: str,
    layer: str,
    description: str,
    columns: list[Column],
    ctes: list[str],
    refs: list[str],
    **extra: object,
) -> Model:
    return project.add(
        Model(
            name=name,
            folder=folder,
            layer=layer,
            description=description,
            columns=columns,
            body=with_query(ctes),
            refs=refs,
            **extra,  # type: ignore[arg-type]
        )
    )


def customer_chain(project: Project, result: Intermediate) -> None:
    orders = result.combined.get("orders_usd")
    identity = result.combined.get("customer_identity")
    if orders is None or identity is None:
        return
    folder = "sql/models/intermediate/customers"
    value = _model(
        project,
        "int_customer_value",
        folder,
        "intermediate",
        "One row per global customer with order count, USD revenue and recency across every "
        "storefront account linked to them",
        [
            Column("global_customer_key", "VARCHAR", ("not_null", "unique"), nullable=False),
            Column("linked_account_count", "INT"),
            Column("order_count", "INT", ("non_negative",)),
            Column("gross_revenue_usd", "NUMBER(18, 2)"),
            Column("first_order_at_utc", "TIMESTAMP_NTZ"),
            Column("last_order_at_utc", "TIMESTAMP_NTZ"),
            Column("days_since_last_order", "INT"),
        ],
        [
            import_cte("orders", f'__ref("{orders.name}")'),
            import_cte("identities", f'__ref("{identity.name}")'),
            cte(
                "linked",
                "SELECT\n"
                + select_list(
                    [
                        "i.global_customer_key",
                        "i.linked_account_count",
                        "o.order_id",
                        "o.placed_at_utc",
                        "o.gross_amount_usd",
                    ]
                )
                + "\nFROM orders AS o\nINNER JOIN identities AS i\n"
                "  ON o.storefront_code = i.storefront_code\n  AND o.customer_id = i.customer_id",
            ),
            cte(
                "final",
                "SELECT\n"
                + select_list(
                    [
                        "l.global_customer_key",
                        "MAX(l.linked_account_count)::INT AS linked_account_count",
                        "COUNT(DISTINCT l.order_id)::INT AS order_count",
                        "SUM(l.gross_amount_usd)::NUMBER(18, 2) AS gross_revenue_usd",
                        "MIN(l.placed_at_utc) AS first_order_at_utc",
                        "MAX(l.placed_at_utc) AS last_order_at_utc",
                        "DATEDIFF('day', MAX(l.placed_at_utc), '@@valuation_date'::DATE)::INT"
                        " AS days_since_last_order",
                    ]
                )
                + "\nFROM linked AS l\nGROUP BY\n  l.global_customer_key",
            ),
        ],
        [orders.name, identity.name],
    )
    segments = _model(
        project,
        "int_customer_segments",
        folder,
        "intermediate",
        "One row per global customer scored on recency, frequency and monetary value in quintiles, "
        "with the segment those scores place them in",
        [
            Column("global_customer_key", "VARCHAR", ("not_null", "unique"), nullable=False),
            Column("order_count", "INT"),
            Column("gross_revenue_usd", "NUMBER(18, 2)"),
            Column("recency_score", "INT"),
            Column("frequency_score", "INT"),
            Column("monetary_score", "INT"),
            Column("segment", "VARCHAR", ("not_null",)),
            Column("is_vip", "BOOLEAN"),
        ],
        [
            import_cte("customer_value", f'__ref("{value.name}")'),
            cte(
                "scored",
                "SELECT\n"
                + select_list(
                    [
                        "v.global_customer_key",
                        "v.order_count",
                        "v.gross_revenue_usd",
                        "NTILE(5) OVER (ORDER BY v.days_since_last_order DESC, "
                        "v.global_customer_key) AS recency_score",
                        "NTILE(5) OVER (ORDER BY v.order_count, v.global_customer_key)"
                        " AS frequency_score",
                        "NTILE(5) OVER (ORDER BY v.gross_revenue_usd, v.global_customer_key)"
                        " AS monetary_score",
                    ]
                )
                + "\nFROM customer_value AS v",
            ),
            cte(
                "final",
                "SELECT\n"
                + select_list(
                    [
                        "s.global_customer_key",
                        "s.order_count",
                        "s.gross_revenue_usd",
                        "s.recency_score::INT AS recency_score",
                        "s.frequency_score::INT AS frequency_score",
                        "s.monetary_score::INT AS monetary_score",
                        "CASE\n    WHEN s.recency_score >= 4 AND s.frequency_score >= 4 "
                        "THEN 'champion'"
                        "\n    WHEN s.recency_score >= 4 THEN 'recent'\n"
                        "    WHEN s.frequency_score >= 4 THEN 'loyal_at_risk'\n"
                        "    WHEN s.recency_score <= 2 AND s.frequency_score <= 2 THEN 'lapsed'\n"
                        "    ELSE 'developing'\n  END::VARCHAR AS segment",
                        '(s.gross_revenue_usd >= @const("vip_ltv_threshold"))::BOOLEAN AS is_vip',
                    ]
                )
                + "\nFROM scored AS s",
            ),
        ],
        [value.name],
    )
    dim = marts.publish(
        project,
        "dim_customers",
        "sql/models/marts/core",
        segments,
        "global_customer_key",
        "One row per global customer with their value scores and segment",
    )
    segment_mix = marts.share_by(
        project,
        "rpt_marketing__segment_mix",
        "sql/models/marts/marketing",
        dim,
        "segment",
        "Global customers",
    )
    vip_mix = marts.share_by(
        project,
        "rpt_marketing__vip_mix",
        "sql/models/marts/marketing",
        dim,
        "is_vip",
        "Global customers",
    )
    result.combined["dim_customers"] = dim
    if segment_mix is not None and vip_mix is not None:
        result.combined["segment_mix"] = segment_mix


def finance_chain(project: Project, result: Intermediate) -> None:
    entries = result.combined.get("ledgers__gl_entries")
    budgets = result.combined.get("ledgers__budgets")
    if entries is None or budgets is None:
        return
    folder = "sql/models/intermediate/ledgers"
    usd = to_usd(
        project,
        "int_gl_entries_usd",
        folder,
        entries,
        ["gl_entry_id"],
        "posted_date_local",
        "Every general ledger entry of every legal entity converted to USD at the reference rate "
        "of its posting day",
    )
    monthly = _model(
        project,
        "int_gl_monthly_usd",
        folder,
        "intermediate",
        "Posted value in USD per legal entity, cost centre and month",
        [
            Column("legal_entity_code", "VARCHAR", ("not_null",)),
            Column("cost_centre", "VARCHAR"),
            Column("period_month", "DATE", ("not_null",)),
            Column("entry_count", "INT"),
            Column("posted_value_usd", "NUMBER(18, 2)"),
        ],
        [
            import_cte("entries", f'__ref("{usd.name}")'),
            cte(
                "final",
                "SELECT\n"
                + select_list(
                    [
                        "e.legal_entity_code",
                        "e.cost_centre",
                        "DATE_TRUNC('month', e.posted_date_local)::DATE AS period_month",
                        "COUNT(*)::INT AS entry_count",
                        "SUM(e.posted_value_amount_usd)::NUMBER(18, 2) AS posted_value_usd",
                    ]
                )
                + "\nFROM entries AS e\nGROUP BY\n  e.legal_entity_code,\n  e.cost_centre,\n"
                "  DATE_TRUNC('month', e.posted_date_local)::DATE",
            ),
        ],
        [usd.name],
    )
    variance = _model(
        project,
        "int_gl_budget_variance",
        folder,
        "intermediate",
        "Actual against budgeted spend per legal entity, cost centre and month, with the variance "
        "as an amount and a percentage of budget",
        [
            Column("variance_key", "VARCHAR", ("not_null", "unique"), nullable=False),
            Column("legal_entity_code", "VARCHAR"),
            Column("cost_centre", "VARCHAR"),
            Column("period_month", "DATE"),
            Column("actual_usd", "NUMBER(18, 2)"),
            Column("budget_amount", "NUMBER(18, 2)"),
            Column("variance_amount", "NUMBER(18, 2)"),
            Column("variance_percent", "NUMBER(9, 4)"),
        ],
        [
            import_cte("actuals", f'__ref("{monthly.name}")'),
            import_cte("budget_lines", f'__ref("{budgets.name}")'),
            cte(
                "budget_months",
                "SELECT\n"
                + select_list(
                    [
                        "b.legal_entity_code",
                        "b.cost_centre",
                        "DATE_TRUNC('month', b.period_date)::DATE AS period_month",
                        "SUM(b.budgeted_amount) AS budget_amount",
                    ]
                )
                + "\nFROM budget_lines AS b\nGROUP BY\n  b.legal_entity_code,\n  b.cost_centre,\n"
                "  DATE_TRUNC('month', b.period_date)::DATE",
            ),
            cte(
                "final",
                "SELECT\n"
                + select_list(
                    [
                        "(a.legal_entity_code || ':' || COALESCE(a.cost_centre, '-') || ':' || "
                        "TO_CHAR(a.period_month, 'YYYY-MM'))::VARCHAR AS variance_key",
                        "a.legal_entity_code",
                        "a.cost_centre",
                        "a.period_month",
                        "a.posted_value_usd AS actual_usd",
                        "COALESCE(m.budget_amount, 0)::NUMBER(18, 2) AS budget_amount",
                        "(a.posted_value_usd - COALESCE(m.budget_amount, 0))::NUMBER(18, 2)"
                        " AS variance_amount",
                        "@sql.percent_of(a.posted_value_usd - m.budget_amount, m.budget_amount)"
                        " AS variance_percent",
                    ]
                )
                + "\nFROM actuals AS a\nLEFT JOIN budget_months AS m\n"
                "  ON a.legal_entity_code = m.legal_entity_code\n"
                "  AND a.cost_centre = m.cost_centre\n  AND a.period_month = m.period_month",
            ),
        ],
        [monthly.name, budgets.name],
    )
    fact = marts.publish(
        project,
        "fct_gl_budget_variance",
        "sql/models/marts/core",
        variance,
        "variance_key",
        "One row per legal entity, cost centre and month with actual and budgeted spend",
    )
    by_entity = marts.share_by(
        project,
        "rpt_finance__variance_by_entity",
        "sql/models/marts/finance",
        fact,
        "legal_entity_code",
        "Budget variance",
    )
    quarterly = marts.period_rollup(
        project,
        "rpt_finance__variance_quarterly",
        "sql/models/marts/finance",
        fact,
        "quarter",
        "Budget variance",
    )
    monthly_report = marts.period_rollup(
        project,
        "rpt_finance__variance_monthly",
        "sql/models/marts/finance",
        fact,
        "month",
        "Budget variance",
    )
    for key, model in (
        ("variance_by_entity", by_entity),
        ("variance_quarterly", quarterly),
        ("variance_monthly", monthly_report),
    ):
        if model is not None:
            result.combined[key] = model


def scorecards(project: Project, result: Intermediate) -> None:
    """Wide per-storefront and per-team scorecards, then executive packs over the scorecards."""
    shop_cards: list[Model] = []
    for code in sorted(project.models):
        if not (code.startswith("rpt_shop_") and code.endswith("__sales_weekly")):
            continue
        shop = code.removeprefix("rpt_").removesuffix("__sales_weekly")
        parts = [
            ("sales", f"rpt_{shop}__sales_weekly"),
            ("payments", f"rpt_{shop}__payments_weekly"),
            ("refunds", f"rpt_{shop}__refunds_weekly"),
        ]
        sources = [(alias, project.models[name]) for alias, name in parts if name in project.models]
        if len(sources) < 2:
            continue
        shop_cards.append(
            marts.wide_daily(
                project,
                f"rpt_{shop}__weekly_scorecard",
                f"sql/models/marts/storefronts/{shop}",
                sources,
                f"One row per week with the sales, payments and refunds of {shop} side by side",
            )
        )
    team_cards: list[Model] = []
    for team in ("finance", "marketing"):
        for region in REGIONS:
            prefix = f"rpt_{team}_{region.code}__"
            weekly = [
                (name.removeprefix(prefix).removesuffix("_weekly"), project.models[name])
                for name in sorted(project.models)
                if name.startswith(prefix) and name.endswith("_weekly")
            ]
            if len(weekly) < 2:
                continue
            team_cards.append(
                marts.wide_daily(
                    project,
                    f"rpt_{team}_{region.code}__scorecard",
                    f"sql/models/marts/{team}/{region.code}",
                    weekly,
                    f"One row per week with every weekly {team} headline for {region.name} "
                    "side by side",
                )
            )
    folder = "sql/models/marts/executive"
    for team in ("finance", "marketing"):
        cards = [card for card in team_cards if card.name.startswith(f"rpt_{team}_")]
        if len(cards) >= 2:
            pack = marts.wide_daily(
                project,
                f"rpt_executive__{team}_regions",
                folder,
                [(card.name.split("_", 2)[2].split("__")[0], card) for card in cards],
                f"One row per period with the first {team} headline of every region side by side",
            )
            result.combined[f"executive_{team}"] = pack
    board = [
        (alias, result.combined[key])
        for alias, key in (
            ("finance", "executive_finance"),
            ("marketing", "executive_marketing"),
            ("variance", "variance_monthly"),
        )
        if key in result.combined
    ]
    if len(board) >= 2:
        marts.wide_daily(
            project,
            "rpt_executive__board_pack",
            folder,
            board,
            "One row per period with the board's headline finance, marketing and budget figures",
        )
