"""Offline checks for the DuckShop synthetic data loaders.

The loaders run against Snowflake, but the rows they return are plain Python, so these tests check
determinism, the shape each managed source declares, and the invariants the staging models and
their audits rely on, without a warehouse.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from collections import Counter
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import pytest
import yaml

PROJECT_DIR = Path(__file__).resolve().parents[1] / "project"
sys.path.insert(0, str(PROJECT_DIR))

from python.loaders import _edge_cases, _world  # noqa: E402
from python.loaders._registry import GENERATORS, generate, scale_from_vars  # noqa: E402

Rows = list[dict[str, object]]


EDGE_SOURCES_FILE = PROJECT_DIR / "sources" / "edge_cases.yml"


def _sources(*, edge: bool) -> dict[str, dict[str, object]]:
    sources: dict[str, dict[str, object]] = {}
    for path in sorted((PROJECT_DIR / "sources").glob("*.yml")):
        if (path == EDGE_SOURCES_FILE) != edge:
            continue
        for source in yaml.safe_load(path.read_text())["sources"]:
            sources[source["name"]] = source
    return sources


SOURCES = _sources(edge=False)
MANAGED = sorted(name for name, source in SOURCES.items() if source.get("managed"))
EDGE_SOURCES = _sources(edge=True)
EDGE_MANAGED = sorted(name for name, source in EDGE_SOURCES.items() if source.get("managed"))


@pytest.fixture(scope="module")
def tables() -> dict[str, Rows]:
    return {name: generate(name) for name in GENERATORS}


def _kind(value: object) -> str | None:
    """The loader's logical type for a value; one column must not mix kinds."""

    if value is None:
        return None
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float | Decimal):
        return "float"
    if isinstance(value, datetime):
        return "timestamp"
    if isinstance(value, date):
        return "date"
    if isinstance(value, dict | list):
        return "json"
    return "string"


def _digest(rows: Rows) -> str:
    return hashlib.sha256(json.dumps(rows, default=str, sort_keys=True).encode()).hexdigest()


def _latest(rows: Rows, key: str) -> list[dict[str, object]]:
    """The staging layer's cdc_latest: the highest `_seq` per raw key, dropping deletes."""

    latest: dict[object, dict[str, object]] = {}
    for row in rows:
        current = latest.get(row[key])
        if current is None or row["_seq"] > current["_seq"]:  # type: ignore[operator]
            latest[row[key]] = row
    return [row for row in latest.values() if row["_op"] != "D"]


def _sku(value: object) -> str:
    return re.sub(r"\s+", "", str(value).strip()).replace("_", "-").upper()


def test_given_project_sources_when_listing_generators_then_each_managed_source_has_one() -> None:
    assert MANAGED, "no managed sources found"
    assert sorted(GENERATORS) == MANAGED
    unmanaged = sorted(set(SOURCES) - set(MANAGED))
    assert unmanaged == ["finance__fx_rates"]


@pytest.mark.parametrize("name", MANAGED)
def test_given_fresh_caches_when_generating_again_then_rows_are_identical(name: str) -> None:
    first = _digest(generate(name))
    _world.build_world.cache_clear()
    for module in ("_storefront", "_fulfilment", "_inventory", "_clickstream", "_support"):
        for value in vars(sys.modules[f"python.loaders.{module}"]).values():
            if hasattr(value, "cache_clear"):
                value.cache_clear()
    assert _digest(generate(name)) == first


@pytest.mark.parametrize("name", MANAGED)
def test_given_generated_rows_when_compared_to_source_yaml_then_shape_matches(
    name: str, tables: dict[str, Rows]
) -> None:
    rows = tables[name]
    declared = [column["name"] for column in SOURCES[name]["columns"]]  # type: ignore[index]
    assert rows, f"{name} generated no rows"
    for row in rows:
        assert list(row) == declared or set(row) == set(declared), (
            f"{name}: {sorted(set(row) ^ set(declared))}"
        )
    for column in declared:
        kinds = {_kind(row[column]) for row in rows} - {None}
        assert len(kinds) <= 1, f"{name}.{column} mixes {kinds}"
    required = [c["name"] for c in SOURCES[name]["columns"] if c.get("nullable") is False]  # type: ignore[index]
    for column in required:
        assert all(row[column] is not None for row in rows), f"{name}.{column} has NULLs"


def _delivery_keys() -> list[tuple[str, str]]:
    """(source, id column) for append-only feeds audited with unique_per_delivery."""

    keys = []
    for name in MANAGED:
        for column in SOURCES[name]["columns"]:  # type: ignore[union-attr]
            if "unique_per_delivery" in (column.get("audits") or []):
                keys.append((name, column["name"]))
    return keys


DELIVERY_KEYS = _delivery_keys()


@pytest.mark.parametrize(("name", "column"), DELIVERY_KEYS, ids=[k[0] for k in DELIVERY_KEYS])
def test_given_append_only_feed_when_generated_then_ids_repeat_only_across_loads(
    name: str, column: str, tables: dict[str, Rows]
) -> None:
    rows = tables[name]
    deliveries = Counter((row[column], row["_loaded_at"]) for row in rows)
    assert max(deliveries.values()) == 1
    ids = Counter(row[column] for row in rows)
    assert len(ids) < len(rows) or name == "fulfilment__carrier_webhooks"


def test_given_append_only_feeds_when_listed_then_all_nine_are_audited() -> None:
    assert len(DELIVERY_KEYS) == 9


@pytest.mark.parametrize(
    "name", [n for n in MANAGED if "_seq" in [c["name"] for c in SOURCES[n]["columns"]]]
)  # type: ignore[index]
def test_given_change_log_when_generated_then_metadata_is_ordered_and_valid(
    name: str, tables: dict[str, Rows]
) -> None:
    rows = tables[name]
    assert [row["_seq"] for row in rows] == list(range(1, len(rows) + 1))
    assert {row["_op"] for row in rows} <= {"I", "U", "D"}
    loaded = [row["_loaded_at"] for row in rows]
    assert all(isinstance(value, datetime) and value.tzinfo is None for value in loaded)
    assert loaded == sorted(loaded)


def test_given_all_change_logs_when_generated_then_updates_deletes_and_late_rows_occur(
    tables: dict[str, Rows],
) -> None:
    ops = Counter(row["_op"] for name in MANAGED for row in tables[name] if "_op" in row)
    assert ops["U"] > 1000 and ops["D"] > 10
    duplicated = 0
    for name in MANAGED:
        seen = Counter(
            json.dumps(
                {k: v for k, v in row.items() if k not in ("_seq", "_loaded_at")},
                default=str,
                sort_keys=True,
            )
            for row in tables[name]
        )
        duplicated += sum(1 for count in seen.values() if count > 1)
    assert duplicated > 100
    # A late order version lands after a newer one, so _seq order disagrees with status time order.
    rank = {
        "pending": 1,
        "paid": 2,
        "packed": 3,
        "shipped": 4,
        "delivered": 5,
        "returned": 6,
        "cancelled": 7,
    }
    last_rank: dict[object, int] = {}
    out_of_order = 0
    for row in tables["storefront__orders"]:
        status_rank = rank[str(row["status"]).lower()]
        if status_rank < last_rank.get(row["order_id"], 0):
            out_of_order += 1
        last_rank[row["order_id"]] = max(status_rank, last_rank.get(row["order_id"], 0))
    assert out_of_order > 5


# Staging keys after normalisation: (source, raw key column, normalised key).
STAGING_KEYS = [
    ("catalogue__brands", "brand_code", lambda r: str(r["brand_code"]).upper()),
    ("catalogue__product_variants", "sku", lambda r: _sku(r["sku"])),
    ("catalogue__bundles", "bundle_sku", lambda r: _sku(r["bundle_sku"])),
    ("catalogue__suppliers", "supplier_code", lambda r: str(r["supplier_code"]).upper()),
    ("storefront__promotions", "promo_code", lambda r: str(r["promo_code"]).strip().upper()),
    ("inventory__warehouses", "warehouse_code", lambda r: str(r["warehouse_code"]).upper()),
    ("storefront__orders", "order_id", lambda r: r["order_id"]),
    ("storefront__customers", "customer_id", lambda r: r["customer_id"]),
]


@pytest.mark.parametrize(
    ("name", "key", "normalise"), STAGING_KEYS, ids=[k[0] for k in STAGING_KEYS]
)
def test_given_latest_versions_when_normalising_keys_then_keys_stay_unique(
    name, key, normalise, tables: dict[str, Rows]
) -> None:
    keys = [normalise(row) for row in _latest(tables[name], key)]
    assert len(keys) == len(set(keys))


# Raw columns that staging audits require to be present on the surviving version.
STAGING_NOT_NULL = {
    "storefront__customers": ["region_code", "created_at"],
    "storefront__orders": ["customer_id", "storefront_code", "currency_code", "placed_at"],
    "storefront__order_lines": ["order_id", "line_no", "sku"],
    "storefront__payments": ["order_id", "amount_minor"],
    "storefront__refunds": ["payment_id", "amount_minor"],
    "storefront__customer_addresses": ["customer_id", "country_code"],
    "storefront__gift_cards": ["currency_code", "initial_amount_minor"],
    "storefront__loyalty_ledger": ["loyalty_account_id", "points_delta", "posted_at"],
    "storefront__order_status_events": ["order_id", "to_status", "changed_at"],
    "fulfilment__shipments": ["order_id", "warehouse_code", "carrier_code"],
    "fulfilment__returns": ["order_id", "sku"],
    "inventory__stock_counts": ["sku", "warehouse_code", "counted_at_local"],
    "support__tickets": ["created_at"],
    "growth__campaign_spend": ["spend_date"],
}


@pytest.mark.parametrize("name", sorted(STAGING_NOT_NULL))
def test_given_latest_versions_when_checking_audited_columns_then_none_are_null(
    name: str, tables: dict[str, Rows]
) -> None:
    key = SOURCES[name]["columns"][0]["name"]  # type: ignore[index]
    for row in _latest(tables[name], key):
        for column in STAGING_NOT_NULL[name]:
            assert row[column] is not None, f"{name}.{column} is NULL for {row[key]}"


ACCEPTED = {
    ("storefront__reviews", "rating"): None,
    ("clickstream__app_events", "platform"): {"phone", "tablet"},
    ("clickstream__email_events", "event_type"): {
        "sent",
        "delivered",
        "open",
        "click",
        "bounce",
        "unsubscribe",
        "complaint",
    },
    ("support__ticket_comments", "author_type"): {"agent", "customer", "system"},
    ("support__csat_responses", "score"): {1, 2, 3, 4, 5},
    ("fulfilment__return_inspections", "grade"): {"A", "B", "C", "SCRAP"},
    ("catalogue__product_media", "media_type"): {"image", "video", "model3d"},
    ("growth__newsletter_subscriptions", "list_status"): {
        "subscribed",
        "unsubscribed",
        "pending",
        "cleaned",
    },
    ("partners__partner_orders", "schema_version"): {1, 2},
}


@pytest.mark.parametrize(
    ("name", "column"), [k for k, v in ACCEPTED.items() if v], ids=lambda value: str(value)
)
def test_given_coded_columns_when_normalised_then_values_are_accepted(
    name: str, column: str, tables: dict[str, Rows]
) -> None:
    accepted = ACCEPTED[(name, column)]
    normalise = {"grade": lambda v: str(v).strip().upper()}.get(
        column, lambda v: v.lower() if isinstance(v, str) else v
    )
    values = {normalise(row[column]) for row in tables[name] if row[column] is not None}
    assert values <= accepted  # type: ignore[operator]


def test_given_related_tables_when_joining_then_every_reference_resolves(
    tables: dict[str, Rows],
) -> None:
    orders = {row["order_id"] for row in tables["storefront__orders"]}
    customers = {row["customer_id"] for row in tables["storefront__customers"]}
    payments = {row["payment_id"] for row in tables["storefront__payments"]}
    addresses = {
        row["address_id"] for row in _latest(tables["storefront__customer_addresses"], "address_id")
    }
    shipments = {row["shipment_id"] for row in tables["fulfilment__shipments"]}
    assert {row["order_id"] for row in tables["storefront__order_lines"]} <= orders
    assert {row["order_id"] for row in tables["storefront__payments"]} <= orders
    assert {row["payment_id"] for row in tables["storefront__refunds"]} <= payments
    assert {row["customer_id"] for row in tables["storefront__orders"]} <= customers
    assert {row["shipping_address_id"] for row in tables["storefront__orders"]} <= addresses
    assert {row["order_id"] for row in tables["fulfilment__shipments"]} <= orders
    assert {row["shipment_id"] for row in tables["fulfilment__shipment_packages"]} <= shipments
    assert {row["order_id"] for row in tables["fulfilment__returns"]} <= orders
    tracking = Counter(
        row["tracking"]["number"] for row in _latest(tables["fulfilment__shipments"], "shipment_id")
    )  # type: ignore[index]
    assert {re.sub(r"[^A-Za-z0-9]", "", str(t)).upper() for t in tracking}.__len__() == len(
        tracking
    )


def test_given_lookup_customer_when_listing_orders_then_history_exists(
    tables: dict[str, Rows],
) -> None:
    lookup = [row for row in tables["storefront__orders"] if row["customer_id"] == "C000001"]
    assert len({row["order_id"] for row in lookup}) >= 5


def test_given_generated_world_when_scanning_then_edge_cases_are_present(
    tables: dict[str, Rows],
) -> None:
    orders = _latest(tables["storefront__orders"], "order_id")
    assert {row["currency_code"] for row in orders} >= {
        "JPY",
        "GBP",
        "KWD",
    }  # 0, 2 and 3 decimal places
    offsets = {row["placed_at"].utcoffset() for row in orders}  # type: ignore[union-attr]
    assert len(offsets) >= 6
    placed = [row["placed_at"] for row in orders]
    # Orders either side of the US and EU spring-forward instants.
    assert any(
        p.astimezone(UTC).replace(tzinfo=None) == datetime(2026, 3, 29, 0, 55) for p in placed
    )  # type: ignore[union-attr]
    assert any(p.astimezone(UTC).replace(tzinfo=None) == datetime(2026, 3, 8, 7, 5) for p in placed)  # type: ignore[union-attr]
    # Partner order messages with a JSON-null id and the id elsewhere.
    nulls = [
        row
        for row in tables["partners__partner_orders"]
        if "orderId" in row["payload"] and row["payload"]["orderId"] is None
    ]  # type: ignore[operator,index]
    assert nulls
    # Returns, refunds and regex-worthy ticket text.
    assert tables["fulfilment__returns"] and tables["storefront__refunds"]
    assert any(
        re.search(r"DS-O-[0-9]+", str(row["body"])) for row in tables["support__ticket_comments"]
    )
    # Web sessions that run past midnight UTC.
    events = [row for row in tables["clickstream__web_events"] if "ts_ms" in row["event"]]  # type: ignore[operator]
    by_visitor: dict[object, list[datetime]] = {}
    for row in events:
        moment = datetime(1970, 1, 1) + timedelta(milliseconds=int(row["event"]["ts_ms"]))  # type: ignore[index]
        by_visitor.setdefault(str(row["event"]["anonymous_id"]).lower(), []).append(moment)  # type: ignore[index]
    crossings = 0
    for moments in by_visitor.values():
        moments.sort()
        crossings += sum(
            1
            for a, b in zip(moments, moments[1:], strict=False)
            if a.date() != b.date() and (b - a) <= timedelta(minutes=30)
        )
    assert crossings >= 10
    # JSON nulls inside payloads, distinct from SQL NULL columns.
    assert any('": null' in json.dumps(row["specs"]) for row in tables["catalogue__products"])


def test_given_larger_scale_when_generating_then_volume_grows() -> None:
    assert scale_from_vars({}) == 1
    assert scale_from_vars({"data_scale": "3"}) == 3
    with pytest.raises(ValueError):
        scale_from_vars({"data_scale": 0})
    small = len(generate("storefront__orders", 1))
    assert len(generate("storefront__orders", 2)) > 1.8 * small


class _FakeContext:
    def __init__(self) -> None:
        self.vars = {"data_scale": "1"}
        self.destination = '"DUCKSHOP"."DEV"."CATALOGUE__PRODUCTS"'
        self.sql: list[str] = []

    def execute_sql(self, sql: str) -> None:
        self.sql.append(sql)

    def result(self, **kwargs: object) -> dict[str, object]:
        return kwargs


@pytest.mark.parametrize(
    "name", ["catalogue__products", "storefront__orders", "inventory__warehouses"]
)
def test_given_loader_context_when_loading_then_table_is_replaced_with_typed_inserts(
    name: str,
) -> None:
    from python.loaders._registry import load

    context = _FakeContext()
    result = load(context, name)
    rows = generate(name)
    assert result == {"metadata": {"rows": len(rows), "scale": 1}}
    create, *inserts = context.sql
    assert create.startswith(f"CREATE OR REPLACE TABLE {context.destination} (")
    assert len(inserts) == -(-len(rows) // 500)
    assert all("PARSE_JSON(column" in sql for sql in inserts)
    assert all("VALUES\n(PARSE_JSON" not in sql for sql in inserts)
    sqlglot = pytest.importorskip("sqlglot")
    for sql in context.sql:
        sqlglot.parse_one(sql, read="snowflake")


def test_given_special_characters_when_rendering_literals_then_they_are_escaped() -> None:
    from python.loaders._writer import literal

    assert literal("it's") == "'it''s'"
    assert literal({"note": "a\nb"}) == '\'{"note": "a\\\\nb"}\''
    assert literal(None) == "NULL"
    assert literal(True) == "TRUE"
    assert literal(date(2026, 3, 31)) == "'2026-03-31'"


def test_given_edge_sources_when_listing_fixed_tables_then_each_has_rows_and_a_loader() -> None:
    loaders = (PROJECT_DIR / "python" / "loaders" / "edge_cases.py").read_text()

    assert EDGE_MANAGED
    assert sorted(_edge_cases.TABLES) == EDGE_MANAGED
    assert not set(EDGE_MANAGED) & set(GENERATORS)
    for name in EDGE_MANAGED:
        assert f"def {name}(ctx: LoaderContext)" in loaders, name


@pytest.mark.parametrize("name", EDGE_MANAGED)
def test_given_edge_rows_when_compared_to_source_yaml_then_shape_matches(name: str) -> None:
    rows = _edge_cases.TABLES[name]
    columns = EDGE_SOURCES[name]["columns"]
    declared = [column["name"] for column in columns]  # type: ignore[attr-defined]

    assert rows, f"{name} has no rows"
    for row in rows:
        assert list(row) == declared, f"{name}: {sorted(set(row) ^ set(declared))}"
    for column in declared:
        kinds = {_kind(row[column]) for row in rows} - {None}
        assert len(kinds) <= 1, f"{name}.{column} mixes {kinds}"
    for column in columns:  # type: ignore[attr-defined]
        values = [row[column["name"]] for row in rows]
        if column.get("nullable") is False:
            assert None not in values, f"{name}.{column['name']} has NULLs"
        if "unique" in (column.get("audits") or []):
            assert len(set(values)) == len(values), f"{name}.{column['name']} repeats"


@pytest.mark.parametrize("scale", ["1", "10"])
def test_given_any_data_scale_when_building_edge_rows_then_they_are_fixed(scale: str) -> None:
    assert scale_from_vars({"data_scale": scale}) == int(scale)
    rebuilt = {
        "edge__partner_payloads": _edge_cases.partner_payloads(),
        "edge__gateway_callbacks": _edge_cases.gateway_callbacks(),
        "edge__price_windows": _edge_cases.price_windows(),
        "edge__customer_profiles": _edge_cases.customer_profiles(),
        "edge__ledger_entries": _edge_cases.ledger_entries(),
        "edge__ticket_notes": _edge_cases.ticket_notes(),
        "edge__partner_listings": _edge_cases.partner_listings(),
        "edge__delivery_updates": _edge_cases.delivery_updates(),
        "edge__basket_lines": _edge_cases.basket_lines(),
        "edge__date_texts": _edge_cases.date_texts(),
        "edge__amount_checks": _edge_cases.amount_checks(),
        "edge__variant_values": _edge_cases.variant_values(),
        "edge__category_links": _edge_cases.category_links(),
        "edge__supplier_notes": _edge_cases.supplier_notes(),
    }

    assert {name: _digest(rows) for name, rows in rebuilt.items()} == {
        name: _digest(rows) for name, rows in _edge_cases.TABLES.items()
    }


def test_given_first_delivery_batch_when_loading_then_it_is_the_first_load_of_the_log() -> None:
    first = _edge_cases.delivery_updates(1)
    full = _edge_cases.delivery_updates(2)
    first_load = min(row["_loaded_at"] for row in full)  # type: ignore[type-var]

    assert first == [row for row in full if row["_loaded_at"] == first_load]
    assert len(first) < len(full)
    later = [row for row in full if row not in first]
    assert {row["_op"] for row in later} == {"I", "U", "D"}
    shipments = {row["shipment_id"]: row["_seq"] for row in first}
    assert any(
        row["shipment_id"] in shipments and row["_seq"] < shipments[row["shipment_id"]]  # type: ignore[operator]
        for row in later
    ), "the second load must carry a late older version"
