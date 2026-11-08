"""A small DuckDB orders project and the SQL macros its E2E tests call."""

from __future__ import annotations

import re
import subprocess
from collections.abc import Mapping
from pathlib import Path

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SEED_COLUMNS: tuple[tuple[str, str], ...] = (
    ("order_id", "BIGINT"),
    ("status", "VARCHAR"),
    ("amount_cents", "BIGINT"),
    ("fee_cents", "BIGINT"),
    ("tax_cents", "BIGINT"),
    ("created_at", "TIMESTAMP"),
    ("updated_at", "TIMESTAMP"),
    ("is_deleted", "BOOLEAN"),
    ("country_code", "VARCHAR"),
)

BASE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
        "[scopes]\nenforce_placement = false\n\n"
        "[[rules.rule_ignores]]\n"
        'rules = ["SQLCLEAN018"]\n'
        'paths = ["macros/sql/**", "models/**"]\n'
        'reason = "The dedup macros rank by updated_at alone; these tests cover expansion."\n'
    ),
    "seeds/raw_orders.csv": (
        "order_id,status,amount_cents,fee_cents,tax_cents,created_at,updated_at,is_deleted,"
        "country_code\n"
        "1,open,1250,100,20,2026-01-01 00:00:00,2026-01-02 00:00:00,false,GB\n"
        "1,open,1300,100,20,2026-01-01 00:00:00,2026-01-03 00:00:00,false,GB\n"
        "2,closed,500,50,5,2026-01-01 00:00:00,2026-01-01 00:00:00,false,FR\n"
        "3,open,900,0,0,2026-01-01 00:00:00,2026-01-04 00:00:00,true,US\n"
    ),
    "seeds/raw_orders.yml": "seeds:\n  - name: raw_orders\n    description: Raw orders.\n"
    "    columns:\n"
    + "".join(
        f"      - name: {name}\n        type: {data_type}\n" for name, data_type in _SEED_COLUMNS
    ),
    "enums/order_status.sql": 'ENUM (name order_status, members (OPEN "open", CLOSED "closed"));\n',
    "constants/supported_countries.sql": (
        'CONSTANT (name supported_countries, value ["GB", "FR", "HK"]);\n'
    ),
    "models/staging/stg_orders.sql": (
        'MODEL (description "Staged orders");\n\nSELECT * FROM __seed("raw_orders")\n'
    ),
}

CENTS_TO_DOLLARS: dict[str, str] = {
    "macros/sql/cents_to_dollars.sql": (
        'MACRO (\n  description "Cents to dollars",\n'
        "  arguments (amount_cents BIGINT, places INT CONST DEFAULT 2),\n"
        "  returns DECIMAL(18, 2),\n);\n\n"
        "CAST(ROUND(@amount_cents / 100, @places) AS DECIMAL(18, 2))\n"
    )
}
NET_AMOUNT: dict[str, str] = {
    "macros/sql/net_amount.sql": (
        'MACRO (\n  description "Net amount in dollars",\n'
        "  arguments (gross_cents BIGINT, refund_cents BIGINT),\n"
        "  returns DECIMAL(18, 2),\n);\n\n"
        "@sql.cents_to_dollars(@gross_cents - COALESCE(@refund_cents, 0))\n"
    )
}
CLEAN_ORDERS: dict[str, str] = {
    "macros/sql/clean_orders.sql": (
        'MACRO (\n  description "Deduplicated, non-deleted orders",\n'
        "  arguments (src RELATION (order_id BIGINT, updated_at TIMESTAMP, is_deleted BOOLEAN)),\n"
        ");\n\n"
        "WITH live AS (SELECT * FROM @src WHERE NOT is_deleted),\n"
        "ranked AS (\n"
        "  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS "
        "row_num\n"
        "  FROM live\n),\n"
        "latest AS (SELECT * EXCLUDE (row_num) FROM ranked WHERE row_num = 1)\n"
    )
}
AUDIT_COLUMNS: dict[str, str] = {
    "macros/sql/audit_columns.sql": (
        'MACRO (\n  description "Standard audit columns",\n'
        "  arguments (src RELATION (created_at TIMESTAMP, updated_at TIMESTAMP)),\n);\n\n"
        "SELECT @src.created_at, @src.updated_at, @src.updated_at > @src.created_at AS "
        "was_updated\n"
    )
}
IS_OPEN_STATUS: dict[str, str] = {
    "macros/sql/is_open_status.sql": (
        'MACRO (\n  description "Whether a status is still open",\n'
        "  arguments (status VARCHAR, closed ENUM order_status DEFAULT "
        '@enum("order_status").CLOSED),\n'
        "  returns BOOLEAN,\n);\n\n@status <> @closed\n"
    )
}
IN_SUPPORTED_COUNTRY: dict[str, str] = {
    "macros/sql/in_supported_country.sql": (
        'MACRO (\n  description "Whether a country code is supported",\n'
        '  arguments (country VARCHAR, countries LIST VARCHAR DEFAULT ["GB", "FR"]),\n'
        "  returns BOOLEAN,\n);\n\n@country IN @countries\n"
    )
}
LATEST_PER_KEY: dict[str, str] = {
    "macros/sql/latest_per_key.sql": (
        'MACRO (\n  description "Latest row per key",\n'
        "  arguments (\n    src RELATION (updated_at TIMESTAMP, keys COLUMNS),\n  "
        "),\n);\n\n"
        "WITH latest_ranked AS (\n"
        "  SELECT @src.*, ROW_NUMBER() OVER (PARTITION BY @src.keys ORDER BY updated_at DESC) AS "
        "row_num\n"
        "  FROM @src\n),\n"
        "latest_rows AS (SELECT * EXCLUDE (row_num) FROM latest_ranked WHERE row_num = 1)\n"
    )
}
BRACKET: dict[str, str] = {
    "macros/python/labels.py": (
        "def bracket(expression: str) -> str:\n"
        '    """Wrap an expression in brackets."""\n'
        "    return f\"'[' || CAST({expression} AS VARCHAR) || ']'\"\n"
    )
}
ALL_MACROS: dict[str, str] = {
    **CENTS_TO_DOLLARS,
    **NET_AMOUNT,
    **CLEAN_ORDERS,
    **AUDIT_COLUMNS,
    **IS_OPEN_STATUS,
    **IN_SUPPORTED_COUNTRY,
    **LATEST_PER_KEY,
    **BRACKET,
}
MODEL_HEADER: str = 'MODEL (description "Orders");\n\n'
DATABASE: str = "shop.duckdb"


def orders_model(sql: str) -> dict[str, str]:
    """The `orders` mart model with `sql` as its query."""

    return {"models/marts/orders.sql": MODEL_HEADER + sql}


def run_project(
    *, tmp_path: Path, files: Mapping[str, str], command: tuple[str, ...]
) -> tuple[Path, subprocess.CompletedProcess[str]]:
    """Write the base project with `files` over it and run one sqb command against it."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="shop", repo_files={**BASE_FILES, **files}
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *command), project_dir=project_dir
    )
    return project_dir, result


def output(result: subprocess.CompletedProcess[str]) -> str:
    """Everything one command printed."""

    return result.stdout + result.stderr


def inspected_text(*, project_dir: Path, relative_paths: tuple[str, ...]) -> str:
    """The text of every project file a test inspects."""

    return "".join(
        (project_dir / relative_path).read_text(encoding="utf-8")
        for relative_path in relative_paths
    )


WRAPPING_MACROS: dict[str, str] = {
    "macros/python/wrapping.py": (
        "def wrap(expr: str) -> str:\n"
        '    """Wrap an expression in parentheses."""\n'
        '    return f"({expr})"\n\n\n'
        "def pick(mapping: dict) -> str:\n"
        '    """The expression under key a."""\n'
        '    return mapping["a"]\n\n\n'
        "def first(items: list) -> str:\n"
        '    """The first expression of a list."""\n'
        "    return items[0]\n\n\n"
        "def noted(expr: str) -> str:\n"
        '    """An expression and a block comment."""\n'
        '    return f"UPPER({expr}) /* noted */"\n'
    )
}
CHECK_MACRO_PATH: str = "macros/sql/check.sql"


def with_rules(*codes: str) -> dict[str, str]:
    """The base project with the given Rules selected."""

    selected: str = ", ".join(f'"{code}"' for code in codes)
    return {
        "sqlbuild_project.toml": BASE_FILES["sqlbuild_project.toml"]
        + f"\n[rules]\nselect = [{selected}]\n"
    }


def with_ignored_rules(*codes: str, reason: str) -> dict[str, str]:
    """The base project with the given Rules ignored, for cases whose SQL shape is their subject."""

    ignored: str = ", ".join(f'"{code}"' for code in codes)
    return {
        "sqlbuild_project.toml": BASE_FILES["sqlbuild_project.toml"]
        + f'\n[[rules.rule_ignores]]\nrules = [{ignored}]\npaths = ["**"]\nreason = "{reason}"\n'
    }


def check_macro(arguments: str, body: str) -> dict[str, str]:
    """The SQL macro `check` returning a boolean from `body`."""

    return {
        CHECK_MACRO_PATH: (
            f'MACRO (\n  description "A check",\n  arguments ({arguments}),\n'
            f"  returns BOOLEAN,\n);\n\n{body}\n"
        )
    }


def python_normalise_macros(names: list[str]) -> dict[str, str]:
    """One Python macro file per name in the `normalise` bucket of `macros/python/`."""

    return {
        f"macros/python/normalise/{name}.py": (
            f"def {name}(expression: str) -> str:\n"
            f'    """Clean a value with {name}."""\n'
            '    return f"TRIM({expression})"\n'
        )
        for name in names
    }


def sql_text_macros(folder: str, names: list[str]) -> dict[str, str]:
    """One lower-casing SQL macro file per name in `macros/sql/<folder>/`."""

    return {
        f"macros/sql/{folder}/{name}.sql": (
            f'MACRO (description "Tidy {name}", arguments (value VARCHAR), returns VARCHAR);\n\n'
            "LOWER(@value)\n"
        )
        for name in names
    }


def text_macro(name: str, returns: str, body: str, argument: str = "VARCHAR") -> dict[str, str]:
    """The SQL macro `name` taking one `argument`-typed value and declaring `returns`."""

    return _value_macro(name, f"  returns {returns},\n", body, argument)


def untyped_text_macro(name: str, body: str, argument: str = "VARCHAR") -> dict[str, str]:
    """The SQL macro `name` taking one `argument`-typed value without declaring `returns`."""

    return _value_macro(name, "", body, argument)


def _value_macro(name: str, declared: str, body: str, argument: str) -> dict[str, str]:
    """The SQL macro `name` taking one `argument`-typed value, with its `declared` header lines."""

    return {
        f"macros/sql/{name}.sql": (
            f'MACRO (\n  description "Text from a value",\n  arguments (value {argument}),\n'
            f"{declared});\n\n{body}\n"
        )
    }


def contract_model(name: str, columns: str, sql: str) -> dict[str, str]:
    """The mart model `name` with an enforced contract over `columns`."""

    return {
        f"models/marts/{name}.sql": (
            f'MODEL (\n  description "Typed {name}",\n  contract enforced,\n'
            f"  columns ({columns}),\n);\n\n{sql}"
        )
    }


def typed_position_files(shape: str, sql: str, declared: str) -> dict[str, str]:
    """The `orders` model over `sql`, contracted itself, contracted downstream, or uncontracted."""

    downstream: dict[str, str] = contract_model(
        "order_x", f"x (type {declared})", 'SELECT b.x FROM __ref("orders") AS b\n'
    )
    shapes: dict[str, dict[str, str]] = {
        "contracted": contract_model("orders", f"x (type {declared})", sql),
        "downstream": {**orders_model(sql), **downstream},
        "uncontracted": orders_model(sql),
    }
    return shapes[shape]


def lineage_edges(printed: str) -> list[str]:
    """The `source -> target` edges a list-format lineage command printed."""

    return [edge.strip() for edge in re.findall(r"^.*->.*$", printed, flags=re.MULTILINE)]


def adapter_project(adapter: str) -> dict[str, str]:
    """The orders project configured for `adapter`, compiled without a warehouse."""

    return {
        "sqlbuild_project.toml": (
            f'name = "shop"\nadapter = "{adapter}"\ndefault_target = "dev"\n\n'
            '[targets.dev]\ndatabase = "warehouse"\nschema = "analytics"\n\n'
            "[scopes]\nenforce_placement = false\n"
        )
    }


def picked_orders(body: str) -> dict[str, str]:
    """The CTE-list macro `pick` over relation `src` with `body`, called once as `__cte := a`."""

    return {
        "macros/sql/pick.sql": (
            'MACRO (\n  description "Picked orders",\n'
            "  arguments (src RELATION (order_id BIGINT)),\n);\n\n" + body + "\n"
        ),
        **orders_model(
            'WITH raw AS (SELECT * FROM __ref("stg_orders")),\n'
            "@sql.pick(raw, __cte := a)\nSELECT * FROM a\n"
        ),
    }


WEEKLY_ORDERS: dict[str, str] = {
    "seeds/weekly_orders.csv": (
        "week,product_id,region,orders_placed,refund_count\n"
        "1,10,EU,4,1\n1,10,US,6,0\n1,20,EU,5,2\n2,20,US,8,2\n"
    ),
    "seeds/weekly_orders.yml": (
        "seeds:\n  - name: weekly_orders\n    description: Weekly orders per product and region.\n"
        "    columns:\n"
        "      - name: week\n        type: BIGINT\n"
        "      - name: product_id\n        type: BIGINT\n"
        "      - name: region\n        type: VARCHAR\n"
        "      - name: orders_placed\n        type: BIGINT\n"
        "      - name: refund_count\n        type: BIGINT\n"
    ),
}


def rates_macro(name: str, arguments: str, keys: tuple[str, str], orders: str) -> dict[str, str]:
    """A CTE-list macro of weekly refund rates reading its grain as `keys` (aliased, plain)."""

    aliased, plain = keys
    return {
        f"macros/sql/{name}.sql": (
            f'MACRO (\n  description "Weekly refund rates",\n  arguments ({arguments}),\n);\n\n'
            "WITH counted AS (\n"
            "  SELECT\n"
            f"    s.{aliased.format(alias='s')},\n"
            f"    s.{orders} AS orders,\n"
            "    s.refund_count AS refunds,\n"
            f"    ROW_NUMBER() OVER (PARTITION BY {plain} ORDER BY s.week DESC) AS week_rank\n"
            "  FROM @weekly AS s\n"
            "),\n\n"
            "rated AS (\n"
            "  SELECT\n"
            f"    {plain},\n"
            "    SUM(refunds) / NULLIF(SUM(orders), 0) AS refund_rate,\n"
            "    MAX(week_rank) AS weeks\n"
            "  FROM counted\n"
            f"  GROUP BY {plain}\n"
            "),\n\n"
            "rates_out AS (\n"
            f"  SELECT {plain}, refund_rate, weeks\n"
            "  FROM rated\n"
            ")\n"
        )
    }


SLOT_MACRO: dict[str, str] = rates_macro(
    "weekly_rates",
    "weekly RELATION (keys COLUMNS, order_count BIGINT, refund_count BIGINT)",
    ("@weekly.keys", "@weekly.keys"),
    "@weekly.order_count",
)
PER_PRODUCT_MACRO: dict[str, str] = rates_macro(
    "weekly_rates_per_product",
    "weekly RELATION (product_id BIGINT, orders_placed BIGINT, refund_count BIGINT)",
    ("product_id", "product_id"),
    "orders_placed",
)
PER_PRODUCT_REGION_MACRO: dict[str, str] = rates_macro(
    "weekly_rates_per_product_region",
    "weekly RELATION (product_id BIGINT, region VARCHAR, orders_placed BIGINT, "
    "refund_count BIGINT)",
    ("product_id, {alias}.region", "product_id, region"),
    "orders_placed",
)


def rates_model(name: str, per_product: str, per_product_region: str) -> dict[str, str]:
    """A model reading weekly refund rates at two grains through the two given macro calls."""

    return {
        f"models/marts/{name}.sql": (
            'MODEL (description "Refund rates per product, and per product and region");\n\n'
            'WITH weekly AS (SELECT * FROM __seed("weekly_orders")),\n'
            f"{per_product}\n"
            f"{per_product_region}\n"
            "final AS (\n"
            "  SELECT product_id, 'ALL' AS region, refund_rate FROM by_product\n"
            "  UNION ALL\n"
            "  SELECT product_id, region, refund_rate FROM by_product_region\n"
            ")\n"
            "SELECT * FROM final\n"
        )
    }


BY_SLOT: dict[str, str] = rates_model(
    "rates_by_slot",
    "@sql.weekly_rates(weekly := weekly, weekly.keys := (product_id), "
    "weekly.order_count := orders_placed, __cte := by_product)",
    "@sql.weekly_rates(weekly := weekly, weekly.keys := (product_id, region), "
    "weekly.order_count := orders_placed, __cte := by_product_region)",
)
BY_HAND: dict[str, str] = rates_model(
    "rates_by_hand",
    "@sql.weekly_rates_per_product(weekly, __cte := by_product)",
    "@sql.weekly_rates_per_product_region(weekly, __cte := by_product_region)",
)


def slot_model(call: str) -> dict[str, str]:
    """A model calling the weekly rates macro once with `call`'s mappings."""

    return {
        "models/marts/rates.sql": (
            'MODEL (description "Refund rates");\n\n'
            'WITH weekly AS (SELECT * FROM __seed("weekly_orders")),\n'
            f"@sql.weekly_rates(weekly, {call})\n"
            "SELECT * FROM rates_out\n"
        )
    }


ORDER_COLUMNS: dict[str, str] = {
    "macros/sql/order_columns.sql": (
        'MACRO (\n  description "Grain and order count columns",\n'
        "  arguments (weekly RELATION (keys COLUMNS, order_count BIGINT)),\n);\n\n"
        "SELECT @weekly.keys, @weekly.order_count AS orders\n"
    )
}
UNNAMED_COUNT: dict[str, str] = {
    "macros/sql/order_columns.sql": (
        'MACRO (\n  description "Order count column",\n'
        "  arguments (weekly RELATION (order_count BIGINT)),\n);\n\n"
        "SELECT @weekly.order_count\n"
    )
}


LINEAGE_MODEL: dict[str, str] = {
    "models/marts/order_counts.sql": (
        'MODEL (description "Order counts per product and region");\n\n'
        "SELECT @sql.order_columns(w, weekly.keys := (product_id, region), "
        "weekly.order_count := orders_placed)\n"
        'FROM __seed("weekly_orders") AS w\n'
    )
}


def rate_model(name: str, expression: str) -> dict[str, str]:
    """A model whose one `refund_rate` column is `expression` and is declared `VARCHAR`."""

    return {
        f"models/marts/{name}.sql": (
            'MODEL (\n  description "Refund rates",\n'
            "  contract enforced,\n  columns (refund_rate (type VARCHAR)),\n);\n\n"
            f"SELECT {expression} AS refund_rate\n"
            'FROM __seed("weekly_orders") AS w\n'
        )
    }


REFUND_RATE_MACRO: dict[str, str] = {
    "macros/sql/refund_rate.sql": (
        'MACRO (\n  description "Refund rate of one row",\n'
        "  arguments (weekly RELATION (order_count BIGINT, refund_count BIGINT)),\n);\n\n"
        "@weekly.refund_count / NULLIF(@weekly.order_count, 0)\n"
    )
}
UNFORMATTED_SLOT_READS: str = (
    'MACRO (description "Rates", arguments (weekly RELATION (keys COLUMNS, '
    "order_count BIGINT)));\n\n"
    "WITH rated AS (select s.@weekly.keys, sum(s.@weekly.order_count) as orders "
    "from @weekly as s group by s.@weekly.keys)\n"
)
HAND_WRITTEN_READS: tuple[tuple[str, str], ...] = (
    ("@weekly.keys", "product_id"),
    ("s.@weekly.order_count", "s.order_count"),
)


def hand_written(text: str) -> str:
    """`text` with each slot read written as the hand-written column it stands for."""

    for slot_read, column in HAND_WRITTEN_READS:
        text = text.replace(slot_read, column)
    return text


REFUNDS: dict[str, str] = {
    "seeds/refunds.csv": "week,product_id,refund_count\n1,10,1\n2,20,2\n",
    "seeds/refunds.yml": (
        "seeds:\n  - name: refunds\n    description: Refunds per week and product.\n"
        "    columns:\n"
        "      - name: week\n        type: BIGINT\n"
        "      - name: product_id\n        type: BIGINT\n"
        "      - name: refund_count\n        type: BIGINT\n"
    ),
}


def refunded_weeks(query: str) -> dict[str, str]:
    """A macro whose `refunded` CTE is `query`, and a model calling it on orders and refunds."""

    return {
        "macros/sql/refunded_weeks.sql": (
            'MACRO (\n  description "Weeks with refunds",\n'
            "  arguments (\n"
            "    weekly RELATION (keys COLUMNS, week BIGINT),\n"
            "    refunds RELATION (week BIGINT),\n"
            "  ),\n"
            ");\n\n"
            f"WITH refunded AS ({query})\n"
        ),
        "models/marts/refunded.sql": (
            'MODEL (description "Refunded weeks");\n\n'
            'WITH weekly AS (SELECT * FROM __seed("weekly_orders")),\n'
            'refunds AS (SELECT * FROM __seed("refunds")),\n'
            "@sql.refunded_weeks(weekly, refunds, weekly.keys := (product_id))\n"
            "SELECT * FROM refunded\n"
        ),
    }


def macro_file(name: str, arguments: str, body: str) -> dict[str, str]:
    """One SQL macro file called `name`, declaring `arguments`, whose body is `body`."""

    return {
        f"macros/sql/{name}.sql": (
            f'MACRO (\n  description "Macro {name}",\n  arguments ({arguments}),\n);\n\n{body}\n'
        )
    }


def scoped_model(macros: dict[str, str], body: str) -> dict[str, str]:
    """Weekly orders, refunds, `macros`, and a model whose query after its two inputs is `body`."""

    return {
        **WEEKLY_ORDERS,
        **REFUNDS,
        **macros,
        "models/marts/scoped.sql": (
            'MODEL (description "Scoped slot reads");\n\n'
            'WITH weekly AS (SELECT * FROM __seed("weekly_orders")),\n'
            'refunds AS (SELECT * FROM __seed("refunds")),\n'
            f"{body}\n"
        ),
    }


SQL_RULES: dict[str, str] = {
    "sqlbuild_project.toml": BASE_FILES["sqlbuild_project.toml"]
    + '\n[rules]\nselect = ["SQLCLEAN028", "SQLCLEAN029"]\n'
}


def rates_with_middle_cte(middle: str) -> dict[str, str]:
    """The docs' weekly refund rates macro with `middle` as a CTE between its two, and a model."""

    return {
        "macros/sql/weekly_rates.sql": (
            'MACRO (\n  description "Weekly refund rate per grain",\n'
            "  arguments (weekly RELATION (keys COLUMNS, order_count BIGINT, refund_count BIGINT)),"
            "\n);\n\n"
            "WITH counted AS (\n"
            "  SELECT\n"
            "    s.@weekly.keys,\n"
            "    s.@weekly.order_count AS orders,\n"
            "    s.@weekly.refund_count AS refunds\n"
            "  FROM @weekly AS s\n"
            "),\n\n"
            f"{middle},\n\n"
            "rated AS (\n"
            "  SELECT\n"
            "    @weekly.keys,\n"
            "    SUM(refunds) / NULLIF(SUM(orders), 0) AS refund_rate\n"
            "  FROM plain\n"
            "  GROUP BY @weekly.keys\n"
            ")\n"
        ),
        "models/marts/rates.sql": (
            'MODEL (description "Refund rates");\n\n'
            'WITH weekly AS (SELECT * FROM __seed("weekly_orders")),\n'
            "@sql.weekly_rates(weekly, weekly.keys := (product_id, region), "
            "weekly.order_count := orders_placed)\n"
            "SELECT * FROM rated\n"
        ),
    }
