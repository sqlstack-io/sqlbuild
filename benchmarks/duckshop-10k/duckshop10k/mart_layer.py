"""Which marts each instance, team and region gets, plus published facts, dimensions, snapshots."""

from __future__ import annotations

from collections.abc import Callable

from duckshop10k import chains, marts
from duckshop10k.core import Column, Model, Project, cte, import_cte, select_list
from duckshop10k.layers import AREA, Intermediate
from duckshop10k.staging import with_query
from duckshop10k.world import PRODUCT_LINES, REGIONS, Instance

# (metric, template, source entity, arguments)
Spec = tuple[str, str, str, tuple[object, ...]]

INSTANCE_MARTS: dict[str, tuple[Spec, ...]] = {
    "storefront": (
        ("sales_weekly", "period", "orders_enriched_daily", ("week",)),
        ("sales_monthly", "period", "orders_enriched_daily", ("month",)),
        ("channel_mix", "share", "orders_enriched_daily", ("channel",)),
        ("payment_method_mix", "share", "payments_daily", ("method",)),
        ("payments_weekly", "period", "payments_daily", ("week",)),
        ("payments_monthly", "period", "payments_daily", ("month",)),
        ("top_customers", "top", "orders_by_customer", ("customer_id", 100)),
        ("customer_cohorts", "cohorts", "orders_by_customer", ("customer_id", "placed_at_utc")),
        ("discounts_by_code", "share", "order_lines_discounts", ("promo_code",)),
        ("order_status_mix", "breakdown", "orders_enriched", ("status",)),
        ("refund_reasons", "classify", "refunds", ("reason_text",)),
        ("utm_source_mix", "share", "orders_enriched", ("utm_source",)),
        ("utm_campaign_top", "top", "orders_enriched", ("utm_campaign", 25)),
        ("device_mix", "share", "orders_enriched", ("device_type",)),
        ("currency_mix", "share", "orders_enriched", ("currency_code",)),
        ("cart_top_visitors", "top", "cart_funnel", ("anonymous_id", 50)),
        ("refunds_weekly", "period", "refunds", ("week",)),
        ("refund_initiators", "share", "refunds", ("initiated_by",)),
        ("orders_by_channel_monthly", "breakdown", "orders_enriched", ("channel",)),
        ("gift_order_share", "share", "orders_enriched", ("is_gift",)),
    ),
    "marketplace": (
        ("orders_weekly", "period", "orders_enriched_daily", ("week",)),
        ("orders_monthly", "period", "orders_enriched_daily", ("month",)),
        ("status_mix", "share", "orders_enriched_daily", ("status",)),
        ("top_buyers", "top", "orders_enriched", ("buyer_ref", 100)),
        ("listing_status_mix", "share", "listings", ("listing_status",)),
        ("ship_country_mix", "share", "orders_enriched", ("ship_country",)),
    ),
    "carrier": (
        ("shipments_weekly", "period", "shipments_enriched_daily", ("week",)),
        ("shipments_monthly", "period", "shipments_enriched_daily", ("month",)),
        ("service_level_mix", "share", "shipments_enriched_daily", ("service_level",)),
        ("top_warehouses", "top", "shipments_enriched", ("warehouse_code", 20)),
        ("tracking_event_mix", "breakdown", "shipments_tracking", ("event_code",)),
    ),
    "warehouse": (
        ("picks_weekly", "period", "picks_daily", ("week",)),
        ("picks_monthly", "period", "picks_daily", ("month",)),
        ("movement_type_mix", "share", "stock_ledger", ("movement_type",)),
        ("top_moving_skus", "top", "stock_ledger", ("sku", 100)),
        ("receipts_monthly", "period", "receipts_enriched", ("month",)),
        ("adjustment_reasons", "share", "adjustments", ("reason_code",)),
        ("movement_reasons", "breakdown", "stock_movements", ("reason_code",)),
    ),
    "store": (
        ("sales_weekly", "period", "transactions_enriched_daily", ("week",)),
        ("sales_monthly", "period", "transactions_enriched_daily", ("month",)),
        ("register_mix", "share", "transactions_enriched_daily", ("register_id",)),
        ("status_mix", "breakdown", "transactions_enriched", ("status",)),
        ("tender_mix", "share", "tenders", ("tender_type",)),
        ("top_skus", "top", "transaction_lines", ("sku", 50)),
    ),
    "supplier": (
        ("purchase_orders_monthly", "period", "purchase_orders_enriched", ("month",)),
        ("top_skus", "top", "purchase_orders_lines", ("sku", 50)),
        ("warehouse_mix", "share", "purchase_orders_enriched", ("warehouse_code",)),
    ),
    "gateway": (
        ("charges_weekly", "period", "charges_enriched_daily", ("week",)),
        ("status_mix", "share", "charges_enriched_daily", ("status",)),
        ("top_orders", "top", "charges_enriched", ("order_id", 100)),
    ),
    "ad_network": (
        ("spend_weekly", "period", "daily_spend_usd", ("week",)),
        ("spend_monthly", "period", "daily_spend_usd", ("month",)),
        ("objective_mix", "share", "campaigns_enriched", ("objective",)),
        ("top_campaigns", "top", "campaigns_enriched", ("campaign_id", 25)),
    ),
    "messaging": (
        ("sends_weekly", "period", "sends_enriched", ("week",)),
        ("top_campaigns", "top", "sends_enriched", ("campaign_id", 50)),
    ),
    "affiliate": (
        ("clicks_weekly", "period", "clicks_enriched", ("week",)),
        ("top_affiliates", "top", "clicks_enriched", ("affiliate_id", 50)),
    ),
    "support": (
        ("tickets_weekly", "period", "tickets_enriched", ("week",)),
        ("channel_mix", "share", "tickets_enriched", ("contact_channel",)),
        ("priority_mix", "breakdown", "tickets_enriched", ("priority",)),
        ("response_by_channel", "share", "first_response", ("contact_channel",)),
    ),
    "ledger": (
        ("gl_monthly", "period", "gl_entries_daily", ("month",)),
        ("account_mix", "share", "gl_entries_daily", ("account_code",)),
        ("payables_by_bucket", "share", "ap_ageing", ("ageing_bucket",)),
        ("receivables_by_bucket", "share", "ar_ageing", ("ageing_bucket",)),
        ("budget_by_cost_centre", "share", "budgets", ("cost_centre",)),
    ),
    "wholesale": (
        ("orders_monthly", "period", "orders_enriched", ("month",)),
        ("top_accounts", "top", "orders_enriched", ("account_id", 50)),
        ("status_mix", "breakdown", "orders_enriched", ("status",)),
        ("invoices_weekly", "period", "invoices_enriched", ("week",)),
    ),
    "returns": (
        ("requests_weekly", "period", "return_requests_enriched", ("week",)),
        ("reason_classes", "classify", "return_requests", ("reason_text",)),
    ),
    "collector": (
        ("sessions_weekly", "period", "sessions", ("week",)),
        ("landing_page_mix", "share", "sessions", ("landing_page_path",)),
        ("referrer_mix", "share", "sessions", ("referrer_host",)),
        ("top_customers", "top", "sessions", ("customer_id", 100)),
    ),
}

TEAM_REGION_MARTS: dict[str, tuple[Spec, ...]] = {
    "finance": (
        ("revenue_weekly", "period", "orders_enriched_daily", ("week",)),
        ("revenue_monthly", "period", "orders_enriched_daily", ("month",)),
        ("payments_monthly", "period", "payments_daily", ("month",)),
        ("payments_weekly", "period", "payments_daily", ("week",)),
        ("payment_method_share", "share", "payments_daily", ("method",)),
        ("refund_reasons", "classify", "refunds", ("reason_text",)),
        ("storefront_share", "share", "orders_enriched", ("storefront_code",)),
        ("order_status_monthly", "breakdown", "orders_enriched", ("status",)),
        ("currency_share", "share", "orders_enriched", ("currency_code",)),
    ),
    "marketing": (
        ("channel_share", "share", "orders_enriched_daily", ("channel",)),
        ("utm_source_share", "share", "orders_enriched", ("utm_source",)),
        ("utm_campaign_top", "top", "orders_enriched", ("utm_campaign", 50)),
        ("device_share", "share", "orders_enriched", ("device_type",)),
        ("customer_cohorts", "cohorts", "orders_by_customer", ("customer_id", "placed_at_utc")),
        ("top_customers", "top", "orders_by_customer", ("customer_id", 200)),
        ("cart_top_customers", "top", "cart_funnel", ("customer_id", 100)),
        ("orders_weekly", "period", "orders_enriched_daily", ("week",)),
        ("refunds_weekly", "period", "refunds", ("week",)),
    ),
    "merchandising": (
        ("top_skus", "top", "order_lines", ("sku", 100)),
        ("storefront_line_share", "share", "order_lines", ("storefront_code",)),
        ("top_orders_by_lines", "top", "order_lines", ("order_id", 100)),
        ("cart_skus_by_visitor", "top", "cart_funnel", ("anonymous_id", 100)),
        ("refund_share_by_storefront", "share", "refunds", ("storefront_code",)),
        ("orders_by_channel_monthly", "breakdown", "orders_enriched", ("channel",)),
    ),
    "support": (
        ("refund_reasons", "classify", "refunds", ("reason_text",)),
        ("refunds_by_initiator", "share", "refunds", ("initiated_by",)),
        ("refunds_monthly", "breakdown", "refunds", ("initiated_by",)),
    ),
}

COMBINED_MARTS: dict[str, tuple[Spec, ...]] = {
    "operations": (
        ("shipments_weekly", "period", "carriers__shipments_enriched_daily", ("week",)),
        ("service_level_share", "share", "carriers__shipments_enriched_daily", ("service_level",)),
        ("carrier_share", "share", "carriers__shipments_enriched", ("carrier_code",)),
        ("tracking_events_monthly", "breakdown", "carriers__shipments_tracking", ("event_code",)),
        ("picks_weekly", "period", "warehouses__picks_daily", ("week",)),
        ("movement_type_share", "share", "warehouses__stock_ledger", ("movement_type",)),
        ("top_moving_skus", "top", "warehouses__stock_ledger", ("sku", 200)),
        ("receipts_monthly", "period", "warehouses__receipts_enriched", ("month",)),
        ("store_sales_weekly", "period", "stores__transactions_enriched_daily", ("week",)),
        ("purchase_orders_monthly", "period", "suppliers__purchase_orders_enriched", ("month",)),
        ("supplier_top_skus", "top", "suppliers__purchase_orders_lines", ("sku", 200)),
        ("returns_weekly", "period", "returns__return_requests_enriched", ("week",)),
    ),
    "support": (
        ("tickets_weekly", "period", "support__tickets_enriched", ("week",)),
        ("channel_share", "share", "support__tickets_enriched", ("contact_channel",)),
        ("response_by_channel", "share", "support__first_response", ("contact_channel",)),
    ),
    "marketing": (
        ("ad_spend_weekly", "period", "advertising__daily_spend_usd", ("week",)),
        ("ad_spend_monthly", "period", "advertising__daily_spend_usd", ("month",)),
        ("ad_network_share", "share", "advertising__campaigns_enriched", ("ad_network_code",)),
        ("messaging_sends_weekly", "period", "messaging__sends_enriched", ("week",)),
        ("affiliate_clicks_weekly", "period", "affiliates__clicks_enriched", ("week",)),
        ("sessions_weekly", "period", "clickstream__sessions", ("week",)),
        ("landing_page_share", "share", "clickstream__sessions", ("landing_page_path",)),
    ),
    "finance": (
        ("gl_monthly", "period", "ledgers__gl_entries_daily", ("month",)),
        ("payables_by_bucket", "share", "ledgers__ap_ageing", ("ageing_bucket",)),
        ("receivables_by_bucket", "share", "ledgers__ar_ageing", ("ageing_bucket",)),
        ("gateway_charges_weekly", "period", "payments__charges_enriched_daily", ("week",)),
        ("marketplace_orders_monthly", "period", "marketplaces__orders_enriched_daily", ("month",)),
        ("wholesale_orders_monthly", "period", "wholesale__orders_enriched", ("month",)),
        ("orders_usd_monthly", "period", "orders_usd", ("month",)),
    ),
    "merchandising": (
        ("product_line_share", "share", "order_lines_catalogued", ("product_line",)),
        ("sku_family_share", "share", "order_lines_catalogued", ("sku_family",)),
        ("top_category_share", "share", "order_lines_catalogued", ("top_category",)),
        ("colour_share", "share", "order_lines_catalogued", ("colour",)),
        ("listing_status_share", "share", "marketplaces__listings", ("listing_status",)),
    ),
}


def _apply(
    project: Project, name: str, folder: str, spec: Spec, source: Model, subject: str
) -> Model | None:
    _metric, template, _entity, args = spec
    templates: dict[str, Callable[..., Model | None]] = {
        "period": marts.period_rollup,
        "share": marts.share_by,
        "top": marts.top_n,
        "breakdown": marts.breakdown,
        "cohorts": marts.cohorts,
        "classify": marts.classify_text,
    }
    return templates[template](project, name, folder, source, *args, subject)


def build_marts(
    project: Project, result: Intermediate, systems: dict[str, tuple[Instance, ...]]
) -> dict[str, Model]:
    published: dict[str, Model] = {}
    for system, values in systems.items():
        for instance in values:
            models = result.per_instance.get(instance.code, {})
            folder = f"sql/models/marts/{AREA[system]}/{instance.code}"
            for spec in INSTANCE_MARTS.get(system, ()):
                source = models.get(spec[2])
                if source is not None:
                    _apply(
                        project,
                        f"rpt_{instance.code}__{spec[0]}",
                        folder,
                        spec,
                        source,
                        f"{spec[2].replace('_', ' ').capitalize()} of {instance.label}",
                    )
    for team, specs in TEAM_REGION_MARTS.items():
        for region in REGIONS:
            models = result.regional.get(region.code)
            if not models:
                continue
            folder = f"sql/models/marts/{team}/{region.code}"
            for spec in specs:
                source = models.get(spec[2])
                if source is not None:
                    _apply(
                        project,
                        f"rpt_{team}_{region.code}__{spec[0]}",
                        folder,
                        spec,
                        source,
                        f"{spec[2].replace('_', ' ').capitalize()} across {region.name}",
                    )
    for team, specs in COMBINED_MARTS.items():
        for spec in specs:
            source = result.combined.get(spec[2])
            if source is not None:
                _apply(
                    project,
                    f"rpt_{team}__{spec[0]}",
                    f"sql/models/marts/{team}",
                    spec,
                    source,
                    f"{spec[2].split('__')[-1].replace('_', ' ').capitalize()} company-wide",
                )
    _product_lines(project, result)
    published.update(_published(project, result))
    chains.customer_chain(project, result)
    chains.finance_chain(project, result)
    chains.scorecards(project, result)
    _executive(project)
    _snapshots(project, result, systems)
    return published


def _product_lines(project: Project, result: Intermediate) -> None:
    for line in PRODUCT_LINES:
        source = result.combined.get(f"{line}__daily_demand")
        if source is None:
            continue
        folder = f"sql/models/marts/merchandising/product_lines/{line}"
        subject = f"{line.title()} product-line demand"
        marts.top_n(project, f"rpt_{line}__top_skus", folder, source, "sku", 50, subject)
        marts.share_by(
            project, f"rpt_{line}__storefront_share", folder, source, "storefront_code", subject
        )


def _published(project: Project, result: Intermediate) -> dict[str, Model]:
    folder = "sql/models/marts/core"
    combined = result.combined
    plan = (
        ("fct_orders", "orders_usd", "order_id", "One row per storefront order with USD amounts"),
        (
            "fct_order_lines",
            "order_lines_catalogued",
            "order_line_id",
            "One row per storefront order line with its product and category",
        ),
        (
            "dim_customer_identities",
            "customer_identity",
            "storefront_customer_key",
            "One row per storefront account with the global customer it belongs to",
        ),
        (
            "fct_marketplace_orders",
            "marketplaces__orders_enriched",
            "partner_order_id",
            "One row per marketplace order with its items, fees, returns and shipments",
        ),
        (
            "fct_shipments",
            "carriers__shipments_enriched",
            "shipment_id",
            "One row per carrier shipment with its tracking events and invoice totals",
        ),
        (
            "fct_store_transactions",
            "stores__transactions_enriched",
            "txn_id",
            "One row per retail store transaction with its lines and tenders",
        ),
        (
            "fct_purchase_orders",
            "suppliers__purchase_orders_enriched",
            "po_id",
            "One row per supplier purchase order with its invoices",
        ),
        (
            "fct_gateway_charges",
            "payments__charges_enriched",
            "charge_id",
            "One row per payment gateway charge with its refunds, disputes and fees",
        ),
        (
            "fct_ad_campaigns",
            "advertising__campaigns_enriched",
            "campaign_id",
            "One row per advertising campaign with its spend and conversions",
        ),
        (
            "fct_support_tickets",
            "support__tickets_enriched",
            "ticket_id",
            "One row per support ticket with its comments, ratings and events",
        ),
        (
            "fct_wholesale_orders",
            "wholesale__orders_enriched",
            "b2b_order_id",
            "One row per wholesale order with its lines and invoices",
        ),
        (
            "fct_return_requests",
            "returns__return_requests_enriched",
            "return_request_id",
            "One row per return request with its labels, inspections and refunds",
        ),
    )
    published: dict[str, Model] = {}
    for name, entity, key, description in plan:
        source = combined.get(entity)
        if source is not None:
            published[name] = marts.publish(project, name, folder, source, key, description)
    catalogue = result.per_instance.get("pim")
    if catalogue is not None:
        published["dim_products"] = marts.publish(
            project,
            "dim_products",
            folder,
            catalogue["products_enriched"],
            "product_id",
            "One row per product with its variant and media counts",
        )
    sessions = combined.get("clickstream__sessions")
    if sessions is not None:
        published["fct_web_sessions"] = marts.publish(
            project,
            "fct_web_sessions",
            folder,
            sessions,
            "session_id",
            "One row per web or app session, merged incrementally with a two-day lookback for "
            "sessions that span midnight",
            config=[
                "materialized incremental",
                "incremental_strategy merge",
                "unique_key [session_id]",
                "cursor session_start_utc",
                "cursor_type timestamp",
                "cursor_grain hour",
                "lookback 2d",
                f"cursor_inputs (\n    {sessions.name} session_start_utc,\n  )",
                "on_schema_change append_new_columns",
            ],
        )
    return published


def _executive(project: Project) -> None:
    folder = "sql/models/marts/executive"
    daily_sources = [
        (region.code, project.models[f"int_storefronts_{region.code}__orders_enriched_daily"])
        for region in REGIONS
        if f"int_storefronts_{region.code}__orders_enriched_daily" in project.models
    ]
    if len(daily_sources) >= 2:
        marts.wide_daily(
            project,
            "rpt_executive__regional_orders_daily",
            folder,
            daily_sources,
            "One row per day with the order count of every storefront region side by side",
        )
    others = [
        ("marketplaces", "int_marketplaces__orders_enriched_daily"),
        ("carriers", "int_carriers__shipments_enriched_daily"),
        ("warehouses", "int_warehouses__picks_daily"),
        ("stores", "int_stores__transactions_enriched_daily"),
        ("gateways", "int_payments__charges_enriched_daily"),
        ("ledgers", "int_ledgers__gl_entries_daily"),
    ]
    channel_sources = [
        (alias, project.models[name]) for alias, name in others if name in project.models
    ]
    channel_sources += daily_sources[:4]
    if len(channel_sources) >= 2:
        marts.wide_daily(
            project,
            "rpt_executive__operations_daily",
            folder,
            channel_sources,
            "One row per day with the headline volume of every operating area side by side",
        )


def _snapshots(
    project: Project, result: Intermediate, systems: dict[str, tuple[Instance, ...]]
) -> None:
    for instance in systems.get("storefront", ())[:8]:
        customers = result.per_instance[instance.code]["customers"]
        tracked = [
            "customer_id",
            "email",
            "country_code",
            "loyalty_tier",
            "is_marketing_opt_in",
            "source_loaded_at",
        ]
        _snapshot(
            project,
            f"snap_{instance.code}__customers",
            customers,
            "customer_id",
            tracked,
            f"Version history of {instance.label} customer accounts",
        )
    catalogue = result.per_instance.get("pim")
    if catalogue is not None:
        _snapshot(
            project,
            "snap_products",
            catalogue["products"],
            "product_id",
            ["product_id", "title", "product_line", "category_path", "source_loaded_at"],
            "Version history of products",
        )
        _snapshot(
            project,
            "snap_price_lists",
            catalogue["price_lists"],
            "price_list_id",
            ["price_list_id", "sku", "currency_code", "list_price_amount", "source_loaded_at"],
            "Version history of list prices, by checking the price for changes",
            check=["list_price_amount"],
        )


def _snapshot(
    project: Project,
    name: str,
    source: Model,
    key: str,
    tracked: list[str],
    description: str,
    check: list[str] | None = None,
) -> None:
    columns = [Column(c, source.column(c).type) for c in tracked]
    columns[0] = Column(
        key, source.column(key).type, ("not_null (run_scope delta_and_final)",), nullable=False
    )
    if check:
        config = [
            "materialized snapshot",
            f"unique_key [{key}]",
            "snapshot_strategy check",
            f"check_columns [{', '.join(check)}]",
        ]
    else:
        config = [
            "materialized snapshot",
            f"unique_key [{key}]",
            "snapshot_strategy timestamp",
            "updated_at source_loaded_at",
            "invalidate_hard_deletes true",
        ]
    ctes = [
        import_cte("source_rows", f'__ref("{source.name}")'),
        cte("final", f"SELECT\n{select_list([f's.{c}' for c in tracked])}\nFROM source_rows AS s"),
    ]
    project.add(
        Model(
            name=name,
            folder="sql/models/snapshots",
            layer="snapshots",
            description=description,
            config=config,
            columns=columns,
            body=with_query(ctes),
            refs=[source.name],
        )
    )
