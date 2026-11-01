"""Macros, enums, constants, generic audits, schemas and functions shared across the project."""

from __future__ import annotations

from duckshop10k.core import Declaration, Project
from duckshop10k.world import CURRENCY_EXPONENT

SQL_MACROS: dict[str, str] = {
    "cdc_latest": """MACRO (
  description "Latest surviving version of each key in an append-only change log",
  arguments (src RELATION (_op VARCHAR, _seq NUMBER(38, 0), _loaded_at TIMESTAMP_NTZ, key COLUMNS)),
);

WITH cdc_versions AS (
  SELECT *
  FROM @src
  QUALIFY ROW_NUMBER() OVER (PARTITION BY @src.key ORDER BY _seq DESC) = 1
),

cdc_latest AS (
  SELECT *
  FROM cdc_versions
  WHERE _op <> @enum("cdc_op").DELETE
)
""",
    "cdc_history": """MACRO (
  description "Every version of each key in a change log, keeping the last one of each load",
  arguments (src RELATION (_op VARCHAR, _seq NUMBER(38, 0), _loaded_at TIMESTAMP_NTZ, key COLUMNS)),
);

WITH cdc_history AS (
  SELECT *
  FROM @src
  QUALIFY ROW_NUMBER() OVER (PARTITION BY @src.key, _loaded_at ORDER BY _seq DESC) = 1
)
""",
    "load_metadata": """MACRO (
  description "Standard lineage columns carried from a change-log row",
  arguments (src RELATION (_seq NUMBER(38, 0), _loaded_at TIMESTAMP_NTZ)),
);

SELECT
  @src._seq AS source_seq,
  TO_TIMESTAMP_NTZ(@src._loaded_at) AS source_loaded_at
""",
    "normalise_email": """MACRO (
  description "Lower-cased, trimmed email address, or NULL when it is not a plausible address",
  arguments (email VARCHAR),
  returns VARCHAR,
);

IFF(
  REGEXP_LIKE(LOWER(TRIM(@email)), @const("email_pattern")),
  REGEXP_REPLACE(LOWER(TRIM(@email)), '\\\\+[^@]*@', '@'),
  NULL
)
""",
    "normalise_phone": """MACRO (
  description "Digits-only phone number with a leading plus, or NULL when too short",
  arguments (phone VARCHAR),
  returns VARCHAR,
);

IFF(LENGTH(REGEXP_REPLACE(@phone, '[^0-9]', '')) >= 7, '+'
|| LTRIM(
  REGEXP_REPLACE(@phone, '[^0-9]', ''),
  '0'
), NULL)
""",
    "normalise_sku": """MACRO (
  description "Canonical SKU: upper case with whitespace removed and underscores as hyphens",
  arguments (sku VARCHAR),
  returns VARCHAR,
);

UPPER(TRANSLATE(
  REGEXP_REPLACE(TRIM(@sku), '\\\\s+', ''),
  '_',
  '-'
))
""",
    "json_text": """MACRO (
  description "Text of a semi-structured value, treating JSON null and blank strings as SQL NULL",
  arguments (value VARIANT),
  returns VARCHAR,
);

NULLIF(
  TRIM(STRIP_NULL_VALUE(@value)::VARCHAR),
  ''
)
""",
    "epoch_ms_to_utc": """MACRO (
  description "UTC timestamp from epoch milliseconds",
  arguments (epoch_ms NUMBER(38, 0)),
  returns TIMESTAMP_NTZ,
);

TO_TIMESTAMP_NTZ(@epoch_ms, 3)
""",
    "safe_divide": """MACRO (
  description "Division that returns NULL instead of failing on a zero denominator",
  arguments (numerator NUMBER(38, 6), denominator NUMBER(38, 6)),
  returns NUMBER(38, 6),
);

(
  @numerator / NULLIF(@denominator, 0)
)::NUMBER(38, 6)
""",
    "money_from_minor": """MACRO (
  description "Converts an integer amount in minor units to major units for a currency exponent",
  arguments (amount NUMBER(38, 0), exponent NUMBER(38, 0), places INT CONST DEFAULT 2),
  returns NUMBER(38, 6),
);

ROUND(@sql.safe_divide(@amount, POWER(10, @exponent)::NUMBER(38, 0)), @places)::NUMBER(38, 6)
""",
    "local_business_date": """MACRO (
  description "Calendar date of a UTC timestamp in an IANA time zone",
  arguments (ts_utc TIMESTAMP_NTZ, tz VARCHAR),
  returns DATE,
);

CONVERT_TIMEZONE('UTC', @tz, @ts_utc)::DATE
""",
    "local_to_utc": """MACRO (
  description "UTC timestamp of a wall-clock time recorded in an IANA time zone",
  arguments (ts_local TIMESTAMP_NTZ, tz VARCHAR),
  returns TIMESTAMP_NTZ,
);

CONVERT_TIMEZONE(@tz, 'UTC', @ts_local)::TIMESTAMP_NTZ
""",
    "tz_to_utc": """MACRO (
  description "UTC wall-clock time of an offset timestamp",
  arguments (ts TIMESTAMP_TZ),
  returns TIMESTAMP_NTZ,
);

CONVERT_TIMEZONE('UTC', @ts)::TIMESTAMP_NTZ
""",
    "status_bucket": """MACRO (
  description "Order status bucket for an order status, used by every sales report",
  arguments (status ENUM order_status),
  returns VARCHAR,
);

CASE
  WHEN @status IN (@enum("order_status").CANCELLED, @enum("order_status").PENDING)
  THEN 'open_or_void'
  WHEN @status = @enum("order_status").RETURNED
  THEN 'returned'
  ELSE 'fulfilled'
END
""",
    "percent_of": """MACRO (
  description "Share of a part in a whole as a percentage, NULL when the whole is zero",
  arguments (part NUMBER(38, 6), whole NUMBER(38, 6), places INT CONST DEFAULT 2),
  returns NUMBER(9, 4),
);

ROUND(100 * @sql.safe_divide(@part, @whole), @places)::NUMBER(9, 4)
""",
    "week_start": """MACRO (
  description "Monday of the ISO week a date falls in",
  arguments (day DATE),
  returns DATE,
);

DATE_TRUNC('week', @day)::DATE
""",
}

PY_MACROS: dict[str, tuple[str, str]] = {
    "union_relations": (
        "relations.py",
        '''"""Set operations over several relations with the same named columns."""

from sqlbuild.refs import SqlResourceRef


def union_relations(relations: list[SqlResourceRef], columns: list[str]) -> str:
    """UNION ALL of the same named columns from each relation, tagged with its relation name."""
    projection = ", ".join(columns)
    return "\\nUNION ALL\\n".join(
        f"SELECT CAST('{relation.name}' AS VARCHAR) AS source_relation, {projection} "
        f"FROM {relation}"
        for relation in relations
    )
''',
    ),
}

ENUMS: dict[str, str] = {
    "cdc_op": """ENUM (
  name cdc_op,
  members (
    INSERT "I",
    UPDATE "U",
    DELETE "D",
  ),
);
""",
    "order_status": """ENUM (
  name order_status,
  members [PENDING, PAID, PACKED, SHIPPED, DELIVERED, CANCELLED, RETURNED],
);
""",
    "sales_channel": """ENUM (
  name sales_channel,
  members (
    WEB "web",
    APP "app",
    STORE "store",
    MARKETPLACE "marketplace",
    WHOLESALE "wholesale",
    PHONE "phone",
  ),
);
""",
    "ticket_priority": """ENUM (
  name ticket_priority,
  members (URGENT 1, HIGH 2, NORMAL 3, LOW 4),
);
""",
}

CONSTANTS: dict[str, str] = {
    "email_pattern": (
        'CONSTANT (name email_pattern, value "^[a-z0-9._%+-]+@[a-z0-9.-]+\\\\.[a-z]{2,}$");\n'
    ),
    "bot_user_agent_pattern": (
        'CONSTANT (name bot_user_agent_pattern, value ".*(bot|crawler|spider|headless).*");\n'
    ),
    "session_gap_minutes": "CONSTANT (name session_gap_minutes, value 30);\n",
    "vip_ltv_threshold": ('CONSTANT (name vip_ltv_threshold, type decimal, value "1500.00");\n'),
    "history_start_date": 'CONSTANT (name history_start_date, value "2025-01-01");\n',
}

AUDITS: dict[str, str] = {
    "valid_currency": """AUDIT ();

WITH currencies AS (
  SELECT *
  FROM __seed("currencies")
),

final AS (
  SELECT r.*
  FROM @relation AS r
  LEFT JOIN currencies AS c
    ON r.@column = c.currency_code
  WHERE
    r.@column IS NOT NULL
    AND c.currency_code IS NULL
)

SELECT *
FROM final
""",
    "non_negative": """AUDIT ();

WITH final AS (
  SELECT *
  FROM @relation
  WHERE @column < 0
)

SELECT *
FROM final
""",
    "has_rows": """AUDIT ();

WITH counted AS (
  SELECT COUNT(*) AS row_count
  FROM @relation
),

final AS (
  SELECT c.row_count
  FROM counted AS c
  WHERE c.row_count < @minimum_rows
)

SELECT *
FROM final
""",
    "match_rate": """AUDIT (
  evaluation measurement,
  value match_rate,
  sample_count total_rows,
  sample_unit rows,
);

MEASURE (
  WITH final AS (
  SELECT
    COUNT(*) AS total_rows,
    100.0 * AVG(IFF(@matched, 1, 0)) AS match_rate
  FROM @relation
)

SELECT *
FROM final
);

EVIDENCE (
  WITH final AS (
  SELECT *
  FROM @relation
  WHERE NOT COALESCE(@matched, FALSE)
)

SELECT *
FROM final
);
""",
}


MACRO_EXCEPTIONS: tuple[tuple[str, str, str], ...] = (
    (
        "SQLCLEAN018",
        "cdc_latest",
        """
The macro cannot see its callers' keys on its own. Every call site is still checked, against the
unique, non-null _seq that each change-log source declares.
""",
    ),
    (
        "SQLCLEAN018",
        "cdc_history",
        """
The macro cannot see its callers' keys on its own. _seq is unique and non-null in every change-log
source, so ordering by it within a key and load is deterministic.
""",
    ),
    (
        "SQLCLEAN034",
        "load_metadata",
        """
A column-list macro body is a select list by construction. Its columns are inserted into the
caller's own CTE, where the rule is checked again.
""",
    ),
)


def declare_all(project: Project) -> None:
    for rule, name, reason in MACRO_EXCEPTIONS:
        project.exceptions.append((rule, ("macro", name), reason))
    for name, content in SQL_MACROS.items():
        project.declare(Declaration(name, "macros/sql", content))
    for name, (file_name, content) in PY_MACROS.items():
        project.declare(Declaration(name, "macros/python", content, file_name))
    for name, content in ENUMS.items():
        project.declare(Declaration(name, "enums", content))
    for name, content in CONSTANTS.items():
        project.declare(Declaration(name, "constants", content))
    for name, content in AUDITS.items():
        project.declare(Declaration(name, "audits/generic", content))


def currencies_seed() -> str:
    rows = ["currency_code,minor_unit_exponent"]
    rows.extend(f"{code},{exponent}" for code, exponent in sorted(CURRENCY_EXPONENT.items()))
    return "\n".join(rows) + "\n"
