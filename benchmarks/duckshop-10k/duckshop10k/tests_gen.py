"""A sampled set of unit tests, a few scenarios, and the custom rule's test, all generated."""

from __future__ import annotations

from duckshop10k.core import Model, Project
from duckshop10k.layers import Intermediate
from duckshop10k.specs import Field, physical_name
from duckshop10k.staging import StagedTable, physical_columns
from duckshop10k.world import CURRENCY_EXPONENT

# One in this many staging dedupe models, conformed models and so on gets a unit test.
SAMPLE_STRIDE = 20


def _null(column_type: str) -> str:
    return f"CAST(NULL AS {column_type})"


def _rows(columns: list[tuple[str, str]], rows: list[dict[str, str]]) -> str:
    """UNION ALL of literal rows; columns missing from a row are typed NULLs."""
    selects = []
    for index, row in enumerate(rows):
        values = []
        for name, column_type in columns:
            value = row.get(name, _null(column_type))
            values.append(f"    {value} AS {name}" if index == 0 else f"    {value}")
        selects.append("  SELECT\n" + ",\n".join(values))
    return "\n  UNION ALL\n".join(selects)


def _expected(names: list[str], rows: list[list[str]]) -> str:
    """Expected rows; every projection is aliased in every branch, as tests require."""
    return "\n  UNION ALL\n".join(
        "  SELECT\n"
        + ",\n".join(f"    {value} AS {name}" for name, value in zip(names, row, strict=True))
        for row in rows
    )


def _test_path(model: Model, behaviour: str) -> str:
    folder = model.folder.removeprefix("sql/models/")
    return f"tests/unit/{folder}/test_{model.name}__{behaviour}.sql"


def _write_test(
    project: Project, model: Model, behaviour: str, fixtures: list[tuple[str, str]], extra: str = ""
) -> None:
    blocks = [f"{name} AS (\n{body}\n)" for name, body in fixtures]
    text = f'TEST (name "{model.name}__{behaviour}");\n\nWITH ' + ",\n\n".join(blocks)
    project.write(_test_path(model, behaviour), text + extra + "\n")


def _instances_fixture(staged: StagedTable) -> tuple[str, str]:
    instance = staged.instance
    return (
        "__seed__source_instances",
        f"  SELECT\n    '{instance.code.upper()}' AS instance_code,\n"
        f"    '{instance.system}' AS system,\n    '{instance.region}' AS region_code,\n"
        f"    '{instance.timezone}' AS timezone,\n    '{instance.currency}' AS currency_code",
    )


def _model_columns(model: Model) -> list[tuple[str, str]]:
    return [(column.name, column.type) for column in model.columns]


def base_dedupe_test(project: Project, staged: StagedTable) -> None:
    instance, archetype = staged.instance, staged.archetype
    source = f"{instance.code}__{archetype.table}"
    model = project.ref(f"base_{source}")
    raw = [
        (name, column_type.replace(",", ", "))
        for name, column_type in physical_columns(archetype, instance.platform)
    ]
    key = raw[0][0]
    loaded = "'2026-03-0{day} 10:00:00'::TIMESTAMP_NTZ"
    if archetype.append:
        columns = [
            *raw,
            ("_seq", "NUMBER(38, 0)"),
            ("_loaded_at", "TIMESTAMP_NTZ"),
            ("_meta", "VARIANT"),
        ]
        rows = [
            {key: "'K1'", "_seq": "1", "_loaded_at": loaded.format(day=1)},
            {key: "'K1'", "_seq": "2", "_loaded_at": loaded.format(day=2)},
            {key: "'K2'", "_seq": "3", "_loaded_at": loaded.format(day=2)},
        ]
        expected = [("'K1'", "2"), ("'K2'", "3")]
        behaviour = "keeps_the_last_delivery_of_each_row"
    else:
        columns = [
            *raw,
            ("_op", "VARCHAR"),
            ("_seq", "NUMBER(38, 0)"),
            ("_loaded_at", "TIMESTAMP_NTZ"),
            ("_meta", "VARIANT"),
        ]
        rows = [
            {key: "'K1'", "_op": "'I'", "_seq": "1", "_loaded_at": loaded.format(day=1)},
            {key: "'K1'", "_op": "'U'", "_seq": "2", "_loaded_at": loaded.format(day=2)},
            {key: "'K2'", "_op": "'I'", "_seq": "3", "_loaded_at": loaded.format(day=1)},
            {key: "'K2'", "_op": "'D'", "_seq": "4", "_loaded_at": loaded.format(day=3)},
        ]
        expected = [("'K1'", "2")]
        behaviour = "keeps_the_latest_live_version"
    expected_rows = _expected([key, "_seq"], [[value, seq] for value, seq in expected])
    _write_test(
        project,
        model,
        behaviour,
        [
            (f"__source__{source}", _rows(columns, rows)),
            (f"__expected__{model.name}", expected_rows),
        ],
    )


def conform_test(project: Project, staged: StagedTable, conformed: Model) -> None:
    """Statuses mapped to the shared vocabulary and minor units converted by currency exponent."""
    instance, archetype = staged.instance, staged.archetype
    platform = instance.platform
    stg = project.ref(staged.stg)
    key = archetype.key
    currency = CURRENCY_EXPONENT[instance.currency]
    if platform == "classic":
        rows = [
            {key: "'K1'", "status_code": "10", "gross_amount": "12.00"},
            {key: "'K2'", "status_code": "90", "gross_amount": "7.00"},
        ]
        expected = [("'K1'", "'PENDING'", "12.00"), ("'K2'", "'CANCELLED'", "7.00")]
    else:
        minor = 10**currency
        rows = [
            {
                key: "'K1'",
                "status": "'shipped'",
                "gross_amount_minor": str(12 * minor),
                "currency_code": f"'{instance.currency}'",
            },
            {
                key: "'K2'",
                "status": "'delivered'",
                "gross_amount_minor": str(7 * minor),
                "currency_code": f"'{instance.currency}'",
            },
        ]
        expected = [("'K1'", "'SHIPPED'", "12.00"), ("'K2'", "'DELIVERED'", "7.00")]
    fixtures = [
        (f"__ref__{stg.name}", _rows(_model_columns(stg), rows)),
        _instances_fixture(staged),
    ]
    if '__seed("currencies")' in conformed.body:
        fixtures.append(
            (
                "__seed__currencies",
                f"  SELECT\n    '{instance.currency}' AS currency_code,\n"
                f"    {currency} AS minor_unit_exponent",
            )
        )
    expected_rows = _expected(
        [key, "status", "gross_amount"],
        [[k, st, f"{a}::NUMBER(18, 2)"] for k, st, a in expected],
    )
    fixtures.append((f"__expected__{conformed.name}", expected_rows))
    _write_test(project, conformed, "maps_statuses_and_converts_amounts", fixtures)


def sequence_test(project: Project, model: Model, source: Model) -> None:
    rows = [
        {
            "order_id": "'O1'",
            "customer_id": "'C1'",
            "placed_at_utc": "'2026-03-01 09:00:00'::TIMESTAMP_NTZ",
            "gross_amount": "10.00",
        },
        {
            "order_id": "'O2'",
            "customer_id": "'C1'",
            "placed_at_utc": "'2026-03-04 09:00:00'::TIMESTAMP_NTZ",
            "gross_amount": "5.00",
        },
        {
            "order_id": "'O3'",
            "customer_id": "CAST(NULL AS VARCHAR)",
            "placed_at_utc": "'2026-03-02 09:00:00'::TIMESTAMP_NTZ",
            "gross_amount": "1.00",
        },
    ]
    expected = _expected(
        [
            "order_id",
            "customer_sequence_number",
            "days_since_previous",
            "is_first",
            "running_gross_amount",
        ],
        [
            ["'O1'", "1", "CAST(NULL AS INT)", "TRUE", "10.00::NUMBER(18, 2)"],
            ["'O2'", "2", "3", "FALSE", "15.00::NUMBER(18, 2)"],
        ],
    )
    _write_test(
        project,
        model,
        "numbers_orders_per_customer_and_skips_guests",
        [
            (f"__ref__{source.name}", _rows(_model_columns(source), rows)),
            (f"__expected__{model.name}", expected),
        ],
    )


def enrich_test(
    project: Project, model: Model, header: Model, children: list[Model], key: str
) -> None:
    fixtures = [
        (f"__ref__{header.name}", _rows(_model_columns(header), [{key: "'H1'"}, {key: "'H2'"}]))
    ]
    for child in children:
        child_key = next(c.name for c in child.columns if "unique" in c.audits)
        fixtures.append(
            (
                f"__ref__{child.name}",
                _rows(
                    _model_columns(child),
                    [{child_key: "'L1'", key: "'H1'"}, {child_key: "'L2'", key: "'H1'"}],
                ),
            )
        )
    counts = [f"{child.name.split('__', 1)[1]}_count" for child in children]
    expected = _expected(
        [key, *counts], [["'H1'", *("2" for _ in counts)], ["'H2'", *("0" for _ in counts)]]
    )
    fixtures.append((f"__expected__{model.name}", expected))
    _write_test(project, model, "counts_children_and_keeps_headers_without_any", fixtures)


def child_flatten_test(project: Project, child: Model, parent: Model, key: str, array: str) -> None:
    rows = [
        {key: "'P1'", array: "ARRAY_CONSTRUCT(OBJECT_CONSTRUCT(), OBJECT_CONSTRUCT())"},
        {key: "'P2'", array: "ARRAY_CONSTRUCT()"},
    ]
    child_key = next(c.name for c in child.columns if "unique" in c.audits)
    expected = _expected([child_key, "element_index"], [["'P1-0'", "0"], ["'P1-1'", "1"]])
    _write_test(
        project,
        child,
        "emits_one_row_per_array_element",
        [
            (f"__ref__{parent.name}", _rows(_model_columns(parent), rows)),
            (f"__expected__{child.name}", expected),
        ],
    )


MACRO_TESTS: dict[str, tuple[str | None, str]] = {
    "tests/unit/macros/sql/test_money_from_minor__converts_by_currency_exponent.sql": (
        "macro",
        """WITH cases AS (
  SELECT
    v.case_name,
    v.amount_minor,
    v.exponent
  FROM VALUES
    ('two_decimals', 1250, 2),
    ('three_decimals_rounded', 12345, 3),
    ('zero_decimals', 1000, 0),
    ('null_amount', NULL, 2) AS v(case_name, amount_minor, exponent)
),

__macro_actual__ AS (
  SELECT
    c.case_name,
    @sql.money_from_minor(c.amount_minor, c.exponent) AS amount
  FROM cases AS c
),

__macro_expected__ AS (
  SELECT
    v.case_name,
    v.amount::NUMBER(38, 6) AS amount
  FROM VALUES
    ('two_decimals', 12.5),
    ('three_decimals_rounded', 12.35),
    ('zero_decimals', 1000),
    ('null_amount', NULL) AS v(case_name, amount)
)
""",
    ),
    "tests/unit/macros/sql/test_percent_of__returns_null_for_a_zero_whole.sql": (
        "macro",
        """WITH cases AS (
  SELECT
    v.case_name,
    v.part,
    v.whole
  FROM VALUES
    ('quarter', 25, 100),
    ('third_rounded', 1, 3),
    ('zero_whole', 5, 0) AS v(case_name, part, whole)
),

__macro_actual__ AS (
  SELECT
    c.case_name,
    @sql.percent_of(c.part, c.whole) AS share_percent
  FROM cases AS c
),

__macro_expected__ AS (
  SELECT
    v.case_name,
    v.share_percent::NUMBER(9, 4) AS share_percent
  FROM VALUES
    ('quarter', 25),
    ('third_rounded', 33.33),
    ('zero_whole', NULL) AS v(case_name, share_percent)
)
""",
    ),
    "tests/unit/functions/sql/test_udf__sku_family__extracts_leading_letters.sql": (
        None,
        """WITH input_values AS (
  SELECT
    v.sku
  FROM VALUES
    ('DUCK-MAL-01'),
    ('decoy-teal-1'),
    ('123-POND'),
    (NULL) AS v(sku)
),

__udf_actual__ AS (
  SELECT
    i.sku,
    __udf("udf__sku_family")(i.sku) AS sku_family
  FROM input_values AS i
),

__udf_expected__ AS (
  SELECT
    v.sku::VARCHAR AS sku,
    v.sku_family::VARCHAR AS sku_family
  FROM VALUES
    ('DUCK-MAL-01', 'DUCK'),
    ('decoy-teal-1', 'DECOY'),
    ('123-POND', NULL),
    (NULL, NULL) AS v(sku, sku_family)
)
""",
    ),
    "tests/unit/functions/python/test_luhn_valid__checks_card_number_checksums.sql": (
        "udf",
        """WITH input_values AS (
  SELECT
    v.digits
  FROM VALUES
    ('79927398713'),
    ('79927398710'),
    ('4629173058461920'),
    ('4000-0566'),
    ('') AS v(digits)
),

__udf_actual__ AS (
  SELECT
    i.digits,
    __udf("luhn_valid")(i.digits) AS is_valid
  FROM input_values AS i
),

__udf_expected__ AS (
  SELECT
    v.digits::VARCHAR AS digits,
    v.is_valid::BOOLEAN AS is_valid
  FROM VALUES
    ('79927398713', TRUE),
    ('79927398710', FALSE),
    ('4629173058461920', TRUE),
    ('4000-0566', NULL),
    ('', NULL) AS v(digits, is_valid)
)
""",
    ),
    "tests/unit/functions/sql/test_table_fn__customer_orders"
    "__returns_nothing_for_unknown_customers.sql": (
        "table_fn",
        """WITH __table_fn_actual__ AS (
  SELECT
    o.order_id,
    o.status
  FROM __table_fn("table_fn__customer_orders")('DS-NO-SUCH-CUSTOMER') AS o
),

__table_fn_expected__ AS (
  SELECT
    CAST(NULL AS VARCHAR) AS order_id,
    CAST(NULL AS VARCHAR) AS status
  WHERE FALSE
)
""",
    ),
}


def build_tests(
    project: Project, tables: dict[str, dict[str, StagedTable]], result: Intermediate
) -> None:
    staged = [table for code in sorted(tables) for _name, table in sorted(tables[code].items())]
    for table in staged[::SAMPLE_STRIDE]:
        base_dedupe_test(project, table)
    for code in sorted(tables):
        models = result.per_instance.get(code, {})
        shop = tables[code].get("orders")
        if shop is None or shop.instance.system != "storefront":
            continue
        conform_test(project, shop, models["orders"])
        sequence_test(project, models["orders_by_customer"], models["orders"])
        enrich_test(
            project,
            models["orders_enriched"],
            models["orders"],
            [models["order_lines"], models["payments"], models["promotion_redemptions"]],
            "order_id",
        )
        lines = tables[code]["order_lines"]
        child_flatten_test(
            project,
            project.ref(lines.children["discounts"]),
            project.ref(lines.stg),
            "order_line_id",
            "discounts",
        )
    for path, (mode, body) in MACRO_TESTS.items():
        if "table_fn__customer_orders" in path and "fct_orders" not in project.models:
            continue
        name = path.rsplit("/", 1)[1].removeprefix("test_").removesuffix(".sql")
        options = f'mode {mode}, name "{name}"' if mode else f'name "{name}"'
        project.write(path, f"TEST ({options});\n\n{body}")


def _source_fixture(staged: StagedTable, rows: list[dict[str, str]]) -> tuple[str, str]:
    """A source fixture; row keys use logical stems and are mapped to the platform's names."""
    instance, archetype = staged.instance, staged.archetype
    names = {archetype.key: physical_name(Field(archetype.key, "key"), instance.platform)}
    names.update(
        {
            item.stem: physical_name(item, instance.platform)
            for item in archetype.fields
            if item.role in ("id", "sku", "qty", "int", "currency")
        }
    )
    rows = [{names.get(column, column): value for column, value in row.items()} for row in rows]
    raw = [
        (name, column_type.replace(",", ", "))
        for name, column_type in physical_columns(archetype, instance.platform)
    ]
    meta = [("_seq", "NUMBER(38, 0)"), ("_loaded_at", "TIMESTAMP_NTZ"), ("_meta", "VARIANT")]
    if not archetype.append:
        meta.insert(0, ("_op", "VARCHAR"))
    return f"__source__{instance.code}__{archetype.table}", _rows([*raw, *meta], rows)


def _scenario(
    project: Project, path: str, description: str, tags: list[str], fixtures: list[tuple[str, str]]
) -> None:
    blocks = [f"{name} AS (\n{body}\n)" for name, body in fixtures]
    tag_list = ", ".join(f'"{tag}"' for tag in tags)
    header = f'SCENARIO (\n  description "{description}",\n  tags [{tag_list}],\n);\n\n'
    project.write(path, header + "WITH " + ",\n\n".join(blocks) + "\n")


def build_scenarios(project: Project, tables: dict[str, dict[str, StagedTable]]) -> None:
    shop = next(
        (
            tables[code]
            for code in sorted(tables)
            if code.startswith("shop_") and tables[code]["orders"].instance.platform == "nest"
        ),
        None,
    )
    if shop is not None:
        instance = shop["orders"].instance
        loaded = "'2026-03-02 06:00:00'::TIMESTAMP_NTZ"
        placed = "'2026-03-01 18:30:00 +01:00'::TIMESTAMP_TZ"
        live = {"_op": "'I'", "_loaded_at": loaded}
        currency = f"'{instance.currency}'"
        fixtures = [
            _source_fixture(
                shop["orders"],
                [
                    {
                        "order_id": "'O1'",
                        "customer_id": "'C1'",
                        "currency_code": currency,
                        "placed_at": placed,
                        "status": "'paid'",
                        **live,
                        "_seq": "1",
                    },
                    {
                        "order_id": "'O2'",
                        "customer_id": "'C2'",
                        "currency_code": currency,
                        "placed_at": placed,
                        "status": "'paid'",
                        **live,
                        "_seq": "2",
                    },
                    {"order_id": "'O2'", "_op": "'D'", "_seq": "3", "_loaded_at": loaded},
                ],
            ),
            _source_fixture(
                shop["order_lines"],
                [
                    {
                        "order_line_id": "'L1'",
                        "order_id": "'O1'",
                        "sku": "'duck-mal-01'",
                        "quantity": "2",
                        **live,
                        "_seq": "1",
                    },
                    {
                        "order_line_id": "'L2'",
                        "order_id": "'O1'",
                        "sku": "'duck-teal-02'",
                        "quantity": "1",
                        **live,
                        "_seq": "2",
                    },
                ],
            ),
            _source_fixture(
                shop["payments"],
                [
                    {
                        "payment_id": "'P1'",
                        "order_id": "'O1'",
                        "currency_code": currency,
                        **live,
                        "_seq": "1",
                    },
                ],
            ),
            _source_fixture(
                shop["promotion_redemptions"],
                [{"_seq": "1", **live, "promotion_redemption_id": "'R1'", "order_id": "'O9'"}],
            ),
            (
                "__seed__currencies",
                f"  SELECT\n    {currency} AS currency_code,\n"
                f"    {CURRENCY_EXPONENT[instance.currency]} AS minor_unit_exponent",
            ),
            _instances_fixture(shop["orders"]),
            (
                f"__expected__int_{instance.code}__orders_enriched",
                _expected(
                    [
                        "order_id",
                        "status",
                        "order_lines_count",
                        "payments_count",
                        "promotion_redemptions_count",
                    ],
                    [["'O1'", "'PAID'", "2", "1", "0"]],
                ),
            ),
        ]
        _scenario(
            project,
            f"tests/scenarios/storefronts/{instance.code}_orders__counts_live_lines_and_payments.sql",
            "Raw change logs of one storefront flow through dedupe, typing and conforming into "
            "enriched orders, where deleted orders drop out",
            ["storefronts", "orders"],
            fixtures,
        )
    site = next((tables[code] for code in sorted(tables) if code.startswith("wh_")), None)
    if site is not None:
        instance = site["stock_movements"].instance
        loaded = "'2026-03-02 06:00:00'::TIMESTAMP_NTZ"
        local = (
            "'2026-03-01 0{hour}:00:00'::TIMESTAMP_NTZ"
            if instance.platform == "classic"
            else "'2026-03-01 0{hour}:00:00 +00:00'::TIMESTAMP_TZ"
        )
        moment = {"classic": "{stem}_ts", "nest": "{stem}_at"}[instance.platform]
        live = {"_op": "'I'", "_loaded_at": loaded}
        fixtures = [
            _source_fixture(
                site["receipts"],
                [
                    {
                        "receipt_id": "'RC1'",
                        "sku": "'duck-mal-01'",
                        "quantity": "10",
                        moment.format(stem="received"): local.format(hour=1),
                        **live,
                        "_seq": "1",
                    }
                ],
            ),
            _source_fixture(
                site["picks"],
                [
                    {
                        "pick_id": "'PK1'",
                        "sku": "'duck-mal-01'",
                        "quantity": "3",
                        moment.format(stem="picked"): local.format(hour=3),
                        **live,
                        "_seq": "1",
                    }
                ],
            ),
            _source_fixture(
                site["adjustments"],
                [
                    {
                        "adjustment_id": "'AD1'",
                        "sku": "'duck-mal-01'",
                        "qty_delta": "-1",
                        moment.format(stem="adjusted"): local.format(hour=4),
                        **live,
                        "_seq": "1",
                    }
                ],
            ),
            _instances_fixture(site["receipts"]),
            _source_fixture(
                site["stock_movements"],
                [
                    {
                        "movement_id": "'MV1'",
                        "sku": "'duck-mal-01'",
                        "qty_delta": "5",
                        moment.format(stem="moved"): local.format(hour=2),
                        "_seq": "1",
                        "_loaded_at": loaded,
                    }
                ],
            ),
            (
                f"__expected__int_{instance.code}__stock_ledger",
                _expected(
                    ["movement_key", "qty_delta", "on_hand_after"],
                    [
                        ["'receipt:RC1'", "10", "10"],
                        ["'movement:MV1'", "5", "15"],
                        ["'pick:PK1'", "-3", "12"],
                        ["'adjustment:AD1'", "-1", "11"],
                    ],
                ),
            ),
        ]
        _scenario(
            project,
            f"tests/scenarios/warehouses/{instance.code}_stock__runs_the_ledger_in_time_order.sql",
            "Receipts, movements, picks and adjustments of one warehouse become a signed ledger "
            "with the running quantity on hand",
            ["warehouses", "inventory"],
            fixtures,
        )
