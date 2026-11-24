"""Which intermediate models each source system gets, and the unions that combine instances."""

from __future__ import annotations

from dataclasses import dataclass, field

from duckshop10k import domain
from duckshop10k.core import Model, Project
from duckshop10k.intermediate import (
    child_detail,
    conform,
    daily,
    enrich,
    instance_column,
    sequence,
    to_usd,
    union,
)
from duckshop10k.staging import StagedTable
from duckshop10k.world import PRODUCT_LINES, REGIONS, Instance


@dataclass(frozen=True)
class Plan:
    conform: tuple[str, ...]
    enrich: tuple[tuple[str, str, tuple[str, ...]], ...] = ()
    sequences: tuple[tuple[str, str, str, str, str | None], ...] = ()
    daily: tuple[tuple[str, str, str | None], ...] = ()
    children: tuple[tuple[str, str, str], ...] = ()
    unions: tuple[str, ...] = ()


PLANS: dict[str, Plan] = {
    "storefront": Plan(
        conform=(
            "customers",
            "customer_addresses",
            "orders",
            "order_lines",
            "payments",
            "refunds",
            "carts",
            "promotion_redemptions",
        ),
        enrich=(("orders", "order_id", ("order_lines", "payments", "promotion_redemptions")),),
        sequences=(("orders", "order_id", "customer_id", "placed_at_utc", "gross_amount"),),
        daily=(
            ("orders_enriched", "placed_date_local", "channel"),
            ("payments", "authorised_date_local", "method"),
        ),
        children=(("order_lines", "discounts", "order_line_id"),),
        unions=(
            "orders_enriched",
            "orders_by_customer",
            "orders_enriched_daily",
            "payments_daily",
            "customer_profile",
            "order_lines",
            "cart_funnel",
            "refunds",
        ),
    ),
    "marketplace": Plan(
        conform=("orders", "order_items", "fees", "returns", "shipments", "listings"),
        enrich=(
            (
                "orders",
                "partner_order_id",
                ("order_items", "fees", "returns", "shipments"),
            ),
        ),
        daily=(("orders_enriched", "placed_date_local", "status"),),
        unions=("orders_enriched", "orders_enriched_daily", "listings"),
    ),
    "carrier": Plan(
        conform=("shipments", "tracking_events", "invoices"),
        enrich=(("shipments", "shipment_id", ("tracking_events", "invoices")),),
        daily=(("shipments_enriched", "shipped_date_local", "service_level"),),
        children=(("shipments", "tracking", "shipment_id"),),
        unions=("shipments_enriched", "shipments_enriched_daily", "shipments_tracking"),
    ),
    "warehouse": Plan(
        conform=("receipts", "picks", "adjustments", "stock_movements", "putaways"),
        enrich=(("receipts", "receipt_id", ("putaways",)),),
        daily=(("picks", "picked_date_local", None),),
        unions=("stock_ledger", "receipts_enriched", "picks_daily"),
    ),
    "store": Plan(
        conform=("transactions", "transaction_lines", "tenders"),
        enrich=(("transactions", "txn_id", ("transaction_lines", "tenders")),),
        daily=(("transactions_enriched", "opened_date_local", "register_id"),),
        unions=("transactions_enriched", "transactions_enriched_daily"),
    ),
    "supplier": Plan(
        conform=("purchase_orders", "invoices"),
        enrich=(("purchase_orders", "po_id", ("invoices",)),),
        children=(("purchase_orders", "lines", "po_id"),),
        unions=("purchase_orders_enriched", "purchase_orders_lines"),
    ),
    "gateway": Plan(
        conform=("charges", "refunds", "disputes", "fees"),
        enrich=(("charges", "charge_id", ("refunds", "disputes", "fees")),),
        daily=(("charges_enriched", "created_date_local", "status"),),
        unions=("charges_enriched", "charges_enriched_daily"),
    ),
    "ad_network": Plan(
        conform=("campaigns", "ad_groups", "daily_spend", "conversions"),
        enrich=(("campaigns", "campaign_id", ("ad_groups", "daily_spend", "conversions")),),
        unions=("campaigns_enriched", "daily_spend_usd"),
    ),
    "messaging": Plan(
        conform=("sends", "opens", "clicks", "bounces"),
        enrich=(("sends", "send_id", ("opens", "clicks", "bounces")),),
        unions=("sends_enriched",),
    ),
    "affiliate": Plan(
        conform=("clicks", "conversions"),
        enrich=(("clicks", "affiliate_click_id", ("conversions",)),),
        unions=("clicks_enriched",),
    ),
    "support": Plan(
        conform=("tickets", "ticket_comments", "csat_responses", "ticket_events"),
        enrich=(("tickets", "ticket_id", ("ticket_comments", "csat_responses", "ticket_events")),),
        unions=("tickets_enriched", "first_response"),
    ),
    "ledger": Plan(
        conform=("gl_entries", "ap_invoices", "ar_invoices", "budgets"),
        daily=(("gl_entries", "posted_date_local", "account_code"),),
        unions=("gl_entries", "gl_entries_daily", "ap_ageing", "ar_ageing", "budgets"),
    ),
    "wholesale": Plan(
        conform=("accounts", "orders", "order_lines", "invoices", "payments"),
        enrich=(
            ("orders", "b2b_order_id", ("order_lines", "invoices")),
            ("invoices", "b2b_invoice_id", ("payments",)),
        ),
        unions=("orders_enriched", "invoices_enriched"),
    ),
    "returns": Plan(
        conform=("return_requests", "return_labels", "inspections", "refunds_issued"),
        enrich=(
            (
                "return_requests",
                "return_request_id",
                ("return_labels", "inspections", "refunds_issued"),
            ),
        ),
        unions=("return_requests_enriched",),
    ),
    "catalogue": Plan(
        conform=("products", "product_variants", "categories", "price_lists", "media"),
        enrich=(("products", "product_id", ("product_variants", "media")),),
    ),
}

AREA = {
    "storefront": "storefronts",
    "marketplace": "marketplaces",
    "carrier": "carriers",
    "warehouse": "warehouses",
    "store": "stores",
    "supplier": "suppliers",
    "gateway": "payments",
    "ad_network": "advertising",
    "messaging": "messaging",
    "affiliate": "affiliates",
    "collector": "clickstream",
    "support": "support",
    "ledger": "ledgers",
    "wholesale": "wholesale",
    "returns": "returns",
    "catalogue": "catalogue",
}

# Unions that go through the Python union macro rather than hand-written UNION ALL branches.
MACRO_UNIONS = frozenset({"orders_enriched", "shipments_enriched", "charges_enriched"})


@dataclass
class Intermediate:
    """Models the mart layer reads, by instance code and by area."""

    per_instance: dict[str, dict[str, Model]] = field(default_factory=dict)
    regional: dict[str, dict[str, Model]] = field(default_factory=dict)
    combined: dict[str, Model] = field(default_factory=dict)


def _instance_models(
    project: Project, instance: Instance, tables: dict[str, StagedTable], folder: str
) -> dict[str, Model]:
    plan = PLANS[instance.system]
    models: dict[str, Model] = {}
    prefix = f"int_{instance.code}__"
    for table in plan.conform:
        models[table] = conform(project, tables[table], folder)
    for header, key, children in plan.enrich:
        models[f"{header}_enriched"] = enrich(
            project,
            f"{prefix}{header}_enriched",
            folder,
            models[header],
            key,
            [models[child] for child in children],
            f"One row per {header.removesuffix('s').replace('_', ' ')} from {instance.label} "
            f"with totals from its {', '.join(c.replace('_', ' ') for c in children)}",
        )
    for table, key, parent, moment, measure in plan.sequences:
        models[f"{table}_by_{parent.removesuffix('_id')}"] = sequence(
            project,
            f"{prefix}{table}_by_{parent.removesuffix('_id')}",
            folder,
            models[table],
            key,
            parent,
            moment,
            measure,
            f"{table.replace('_', ' ').capitalize()} from {instance.label} in order per "
            f"{parent.removesuffix('_id')}, with gaps between them and running totals",
        )
    for source, day, dimension in plan.daily:
        models[f"{source}_daily"] = daily(
            project,
            f"{prefix}{source}_daily",
            folder,
            models[source],
            day,
            dimension,
            f"Daily totals of {source.replace('_', ' ')} from {instance.label}"
            + (f" by {dimension.replace('_', ' ')}" if dimension else ""),
        )
    for parent, child, key in plan.children:
        staged_child = project.ref(tables[parent].children[child])
        models[f"{parent}_{child}"] = child_detail(
            project,
            f"{prefix}{parent}_{child}",
            folder,
            staged_child,
            models[parent],
            key,
            f"{child.capitalize()} on {parent.replace('_', ' ')} from {instance.label}, with the "
            "parent's times and the number of elements per parent",
        )
    if instance.system == "storefront":
        models["customer_profile"] = domain.customer_profile(
            project,
            f"{prefix}customer_profile",
            folder,
            models["customers"],
            models["customer_addresses"],
            project.ref(tables["customers"].children["consents"]),
            instance_column(instance),
        )
        models["cart_funnel"] = domain.cart_funnel(
            project,
            f"{prefix}cart_funnel",
            folder,
            project.ref(tables["carts"].stg),
            models["carts"],
        )
    if instance.system == "warehouse":
        models["stock_ledger"] = domain.stock_ledger(
            project,
            f"{prefix}stock_ledger",
            folder,
            models["stock_movements"],
            models["adjustments"],
            models["receipts"],
            models["picks"],
            instance_column(instance),
        )
    if instance.system == "ad_network":
        models["daily_spend_usd"] = to_usd(
            project,
            f"{prefix}daily_spend_usd",
            folder,
            models["daily_spend"],
            ["spend_id"],
            "spend_date",
            f"Daily spend from {instance.label} converted to USD at the reference rate of the day",
        )
    if instance.system == "support":
        models["first_response"] = domain.first_response(
            project,
            f"{prefix}first_response",
            folder,
            models["tickets"],
            models["ticket_comments"],
            local_time=instance.code == "support_voice",
        )
    if instance.system == "ledger":
        models["ap_ageing"] = domain.ageing(
            project,
            f"{prefix}ap_ageing",
            folder,
            models["ap_invoices"],
            "ap_invoice_id",
            "supplier_ref",
            "invoiced_total_amount",
            instance_column(instance),
            f"Open payable invoices of {instance.label} by days overdue at the valuation date",
        )
        models["ar_ageing"] = domain.ageing(
            project,
            f"{prefix}ar_ageing",
            folder,
            models["ar_invoices"],
            "ar_invoice_id",
            "customer_ref",
            "invoiced_total_amount",
            instance_column(instance),
            f"Open receivable invoices of {instance.label} by days overdue at the valuation date",
        )
    return models


def _key_of(model: Model) -> str | None:
    return next((c.name for c in model.columns if "unique" in c.audits), None)


def build_intermediate(
    project: Project,
    tables: dict[str, dict[str, StagedTable]],
    events: dict[str, tuple[str, list[str]]],
    systems: dict[str, tuple[Instance, ...]],
) -> Intermediate:
    result = Intermediate()
    for system, values in systems.items():
        if system == "collector" or not values:
            continue
        area = AREA[system]
        for instance in values:
            folder = f"sql/models/intermediate/{area}/{instance.code}"
            result.per_instance[instance.code] = _instance_models(
                project, instance, tables[instance.code], folder
            )
        plan = PLANS[system]
        for entity in plan.unions:
            members_by_region: dict[str, list[Model]] = {}
            for instance in values:
                members_by_region.setdefault(instance.region, []).append(
                    result.per_instance[instance.code][entity]
                )
            via_macro = entity in MACRO_UNIONS
            if system == "storefront":
                regional: list[Model] = []
                for region in REGIONS:
                    members = members_by_region.get(region.code, [])
                    if not members:
                        continue
                    model = union(
                        project,
                        f"int_{area}_{region.code}__{entity}",
                        f"sql/models/intermediate/{area}/regions",
                        members,
                        f"{entity.replace('_', ' ').capitalize()} from every {region.name} "
                        "storefront",
                        _key_of(members[0]),
                        via_macro,
                    )
                    result.regional.setdefault(region.code, {})[entity] = model
                    regional.append(model)
                members = regional
                via_macro = False
            else:
                members = [result.per_instance[instance.code][entity] for instance in values]
            if len(members) == 1:
                result.combined[f"{area}__{entity}"] = members[0]
                continue
            result.combined[f"{area}__{entity}"] = union(
                project,
                f"int_{area}__{entity}",
                f"sql/models/intermediate/{area}",
                members,
                f"{entity.replace('_', ' ').capitalize()} from every {area.removesuffix('s')} "
                "instance",
                _key_of(members[0]),
                via_macro,
            )
    _storefront_globals(project, result)
    _clickstream(project, result, events, systems.get("collector", ()))
    return result


def _storefront_globals(project: Project, result: Intermediate) -> None:
    folder = "sql/models/intermediate/storefronts"
    orders = result.combined["storefronts__orders_enriched"]
    result.combined["orders_usd"] = to_usd(
        project,
        "int_orders_usd",
        folder,
        orders,
        ["order_id"],
        "placed_date_local",
        "Every storefront order with its amounts converted to USD at the reference rate on or "
        "before the local order date",
    )
    result.combined["customer_identity"] = domain.customer_identity(
        project,
        "int_customer_identity",
        "sql/models/intermediate/customers",
        result.combined["storefronts__customer_profile"],
    )
    catalogue = result.per_instance.get("pim")
    if catalogue is None:
        return
    lines = result.combined["storefronts__order_lines"]
    result.combined["order_lines_catalogued"] = _catalogued_lines(
        project, folder, lines, catalogue["product_variants"], catalogue["products"]
    )
    for line in PRODUCT_LINES:
        result.combined[f"{line}__daily_demand"] = _line_demand(
            project, line, result.combined["order_lines_catalogued"]
        )


def _catalogued_lines(
    project: Project, folder: str, lines: Model, variants: Model, products: Model
) -> Model:
    from duckshop10k.core import Column, cte, import_cte, select_list
    from duckshop10k.staging import with_query

    items = [
        "l.order_line_id",
        "l.order_id",
        "l.storefront_code",
        "l.sku",
        "l.quantity",
        "l.unit_price_amount",
        "(l.quantity * l.unit_price_amount)::NUMBER(18, 2) AS line_amount",
        "v.product_id",
        "v.colour",
        "p.product_line",
        "@udf_call AS sku_family",
        "SPLIT_PART(p.category_path, '/', 1)::VARCHAR AS top_category",
    ]
    items[10] = '__udf("udf__sku_family")(l.sku)::VARCHAR AS sku_family'
    ctes = [
        import_cte("order_lines", f'__ref("{lines.name}")'),
        import_cte("variants", f'__ref("{variants.name}")'),
        import_cte("products", f'__ref("{products.name}")'),
        cte(
            "final",
            f"SELECT\n{select_list(items)}\nFROM order_lines AS l\nLEFT JOIN variants AS v\n"
            "  ON l.sku = v.sku\nLEFT JOIN products AS p\n  ON v.product_id = p.product_id",
        ),
    ]
    columns = [
        Column("order_line_id", "VARCHAR", ("not_null", "unique"), nullable=False),
        Column("order_id", "VARCHAR"),
        Column("storefront_code", "VARCHAR"),
        Column("sku", "VARCHAR"),
        Column("quantity", "INT", ("non_negative",)),
        Column("unit_price_amount", "NUMBER(18, 2)"),
        Column("line_amount", "NUMBER(18, 2)"),
        Column("product_id", "VARCHAR"),
        Column("colour", "VARCHAR"),
        Column("product_line", "VARCHAR"),
        Column("sku_family", "VARCHAR"),
        Column("top_category", "VARCHAR"),
    ]
    return project.add(
        Model(
            name="int_order_lines_catalogued",
            folder=folder,
            layer="intermediate",
            description=(
                "Every storefront order line with its product, colour, product line,"
                " SKU family and "
                "top-level category"
            ),
            columns=columns,
            body=with_query(ctes),
            refs=[lines.name, variants.name, products.name],
        )
    )


def _line_demand(project: Project, line: str, lines: Model) -> Model:
    from duckshop10k.core import Column, cte, import_cte, select_list
    from duckshop10k.staging import with_query

    items = [
        "l.storefront_code",
        "l.sku",
        "COUNT(DISTINCT l.order_id)::INT AS order_count",
        "SUM(l.quantity)::INT AS units",
        "SUM(l.line_amount)::NUMBER(18, 2) AS line_amount",
        "RANK() OVER (\n    PARTITION BY l.storefront_code\n    ORDER BY"
        " SUM(l.quantity) DESC, l.sku\n"
        "  )::INT AS units_rank_in_storefront",
    ]
    ctes = [
        import_cte("order_lines", f'__ref("{lines.name}")'),
        cte(
            "final",
            f"SELECT\n{select_list(items)}\nFROM order_lines AS l\nWHERE\n"
            f"  l.product_line = '{line.upper()}'\nGROUP BY\n  l.storefront_code,\n  l.sku",
        ),
    ]
    columns = [
        Column("storefront_code", "VARCHAR", ("not_null",)),
        Column("sku", "VARCHAR", ("not_null",)),
        Column("order_count", "INT"),
        Column("units", "INT", ("non_negative",)),
        Column("line_amount", "NUMBER(18, 2)"),
        Column("units_rank_in_storefront", "INT"),
    ]
    return project.add(
        Model(
            name=f"int_{line}__sku_demand",
            folder=f"sql/models/intermediate/product_lines/{line}",
            layer="intermediate",
            description=(
                f"Units and sales of every {line.title()} product-line SKU per storefront, ranked "
                "within the storefront"
            ),
            columns=columns,
            body=with_query(ctes),
            refs=[lines.name],
        )
    )


def _clickstream(
    project: Project,
    result: Intermediate,
    events: dict[str, tuple[str, list[str]]],
    collectors: tuple[Instance, ...],
) -> None:
    session_models: list[Model] = []
    for collector in collectors:
        stream, _typed = events[collector.code]
        model = domain.sessions(
            project,
            f"int_{collector.code}__sessions",
            f"sql/models/intermediate/clickstream/{collector.code}",
            project.ref(stream),
        )
        result.per_instance.setdefault(collector.code, {})["sessions"] = model
        session_models.append(model)
    if len(session_models) > 1:
        result.combined["clickstream__sessions"] = union(
            project,
            "int_clickstream__sessions",
            "sql/models/intermediate/clickstream",
            session_models,
            "Sessions from every web and app collector",
            "session_id",
            False,
        )
