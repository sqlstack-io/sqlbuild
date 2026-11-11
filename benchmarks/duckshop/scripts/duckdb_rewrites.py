"""SQL rewrites that finish Polyglot's Snowflake-to-DuckDB translation of DuckShop's models.

Used by `duckdb_project.py`. Polyglot translates most Snowflake SQL; these rules cover what it
leaves out or translates with different semantics. Semi-structured values are JSON throughout:
Snowflake's VARIANT, OBJECT and ARRAY all become DuckDB JSON. Snowflake functions DuckDB lacks
become project SQL macros named `sf_<function>`, one per argument count.
"""

from __future__ import annotations

import os
import pathlib
import re
from collections.abc import Callable

_UNQUOTED = "TRIM(CAST(x AS VARCHAR), '\"')"
_SNOWFLAKE_DATE_PARTS = (
    "replace(replace(replace(replace(replace(replace(replace(replace(f, 'TZH:TZM', '%z'), "
    "'TZHTZM', '%z'), 'YYYY', '%Y'), 'MM', '%m'), 'DD', '%d'), 'HH24', '%H'), 'MI', '%M'), "
    "'SS', '%S')"
)
# Snowflake's regular expression flags as RE2 inline flags; `e` (extract a group) is handled by
# the callers.
_REGEX_FLAGS = (
    "CASE WHEN contains(f, 'i') THEN '(?i)' ELSE '' END || "
    "CASE WHEN contains(f, 's') THEN '(?s)' ELSE '' END || "
    "CASE WHEN contains(f, 'm') THEN '(?m)' ELSE '' END || p"
)


def _parse(function: str, cast: str) -> str:
    """Parses `x` with the Snowflake format `f`."""
    return f"CAST({function}({_UNQUOTED}, {_SNOWFLAKE_DATE_PARTS}) AS {cast})"


# Each shim's overloads as (parameters, body). A shim with one overload keeps its own name; others
# get the argument count as a suffix, such as sf_to_number_3.
SHIMS: dict[str, list[tuple[str, str]]] = {
    "sf_strip_null_value": [("x", "CASE WHEN CAST(x AS VARCHAR) = 'null' THEN NULL ELSE x END")],
    "sf_try_to_number": [
        ("x", f"CAST(TRY_CAST({_UNQUOTED} AS DECIMAL(38, 0)) AS BIGINT)"),
        ("x, p", f"CAST(TRY_CAST({_UNQUOTED} AS DECIMAL(38, 0)) AS BIGINT)"),
        ("x, p, s", f"TRY_CAST({_UNQUOTED} AS DECIMAL(38, 9))"),
    ],
    "sf_get_path": [("j, p", "CAST(j AS JSON) -> ('$.' || p)")],
    "sf_to_timestamp_ntz": [("x", f"TRY_CAST({_UNQUOTED} AS TIMESTAMP)")],
    "sf_try_to_timestamp_tz": [
        ("x", f"TRY_CAST({_UNQUOTED} AS TIMESTAMPTZ)"),
        ("x, f", _parse("try_strptime", "TIMESTAMPTZ")),
    ],
    "sf_object_keys": [("j", "json_keys(CAST(j AS JSON))")],
    "sf_is_object": [("j", "json_type(CAST(j AS JSON)) = 'OBJECT'")],
    "sf_initcap": [
        (
            "x",
            "array_to_string(list_transform(string_split(lower(x), ' '), "
            "lambda w: upper(w[1]) || w[2:]), ' ')",
        )
    ],
    "sf_array_size": [("x", "CAST(json_array_length(to_json(x)) AS BIGINT)")],
    "sf_regexp_substr": [
        ("s, p", "NULLIF(regexp_extract(s, p), '')"),
        ("s, p, pos", "NULLIF(regexp_extract(s, p), '')"),
        ("s, p, pos, occ", "NULLIF(regexp_extract(s, p), '')"),
        (
            "s, p, pos, occ, f",
            f"NULLIF(CASE WHEN contains(f, 'e') THEN regexp_extract(s, {_REGEX_FLAGS}, 1) "
            f"ELSE regexp_extract(s, {_REGEX_FLAGS}) END, '')",
        ),
        ("s, p, pos, occ, f, grp", f"NULLIF(regexp_extract(s, {_REGEX_FLAGS}, grp), '')"),
    ],
    "sf_to_varchar": [
        ("x", _UNQUOTED),
        ("x, f", f"strftime(x, {_SNOWFLAKE_DATE_PARTS})"),
    ],
    "sf_to_number": [
        ("x", f"CAST(CAST({_UNQUOTED} AS DECIMAL(38, 0)) AS BIGINT)"),
        ("x, p", f"CAST(CAST({_UNQUOTED} AS DECIMAL(38, 0)) AS BIGINT)"),
        ("x, p, s", f"CAST({_UNQUOTED} AS DECIMAL(38, 9))"),
    ],
    "sf_parse_url": [
        (
            "u, p",
            "{'host': regexp_extract(u, '^[a-zA-Z]+://([^/:?#]+)', 1), "
            "'path': regexp_extract(u, '^[a-zA-Z]+://[^/?#]*(/[^?#]*)?', 1), "
            "'parameters': map_from_entries(list_transform(list_filter("
            "string_split(regexp_extract(u, '[?]([^#]*)', 1), '&'), lambda kv: kv <> ''), "
            "lambda kv: {'key': split_part(kv, '=', 1), 'value': split_part(kv, '=', 2)})), "
            "'query': regexp_extract(u, '\\?([^#]*)', 1), "
            "'scheme': regexp_extract(u, '^([a-zA-Z]+)://', 1)}",
        )
    ],
    "sf_is_decimal": [("x", "json_type(to_json(x)) IN ('DOUBLE', 'UBIGINT', 'BIGINT')")],
    "sf_is_integer": [("x", "json_type(to_json(x)) IN ('UBIGINT', 'BIGINT')")],
    "sf_typeof": [
        (
            "x",
            "CASE json_type(to_json(x)) WHEN 'VARCHAR' THEN 'VARCHAR' WHEN 'BIGINT' THEN 'INTEGER' "
            "WHEN 'UBIGINT' THEN 'INTEGER' WHEN 'DOUBLE' THEN 'DECIMAL' "
            "WHEN 'BOOLEAN' THEN 'BOOLEAN' WHEN 'OBJECT' THEN 'OBJECT' "
            "WHEN 'ARRAY' THEN 'ARRAY' ELSE 'NULL_VALUE' END",
        )
    ],
    "sf_array_contains": [("a, v", "list_contains(CAST(to_json(a) AS JSON[]), to_json(v))")],
    "sf_array_flatten": [("x", "flatten(CAST(to_json(x) AS JSON[][]))")],
    "sf_try_to_timestamp_ntz": [
        ("x", f"TRY_CAST({_UNQUOTED} AS TIMESTAMP)"),
        ("x, f", _parse("try_strptime", "TIMESTAMP")),
    ],
    "sf_to_timestamp_tz": [
        ("x", f"CAST({_UNQUOTED} AS TIMESTAMPTZ)"),
        ("x, f", _parse("strptime", "TIMESTAMPTZ")),
    ],
    "sf_regexp_like": [
        ("s, p", "regexp_full_match(s, p)"),
        ("s, p, f", f"regexp_full_match(s, {_REGEX_FLAGS})"),
    ],
    "sf_array_distinct": [("x", "list_distinct(CAST(to_json(x) AS JSON[]))")],
    "sf_sha2": [("x", "sha256(CAST(x AS VARCHAR))"), ("x, n", "sha256(CAST(x AS VARCHAR))")],
    "sf_dateadd": [("u, n, d", "d + CAST(CAST(n AS VARCHAR) || ' ' || u AS INTERVAL)")],
    "sf_get": [
        (
            "j, k",
            "CASE WHEN json_type(CAST(j AS JSON)) = 'ARRAY' THEN CAST(j AS JSON) -> "
            "TRY_CAST(k AS BIGINT) ELSE CAST(j AS JSON) -> CAST(k AS VARCHAR) END",
        )
    ],
    "sf_object_agg": [
        (
            "k, v",
            "to_json(map_from_entries(ARRAY_AGG({'key': CAST(k AS VARCHAR), 'value': to_json(v)}) "
            "FILTER (WHERE k IS NOT NULL AND v IS NOT NULL)))",
        )
    ],
    "sf_hash": [("x", "CAST(hash(x) >> 1 AS BIGINT)")],
    "sf_list_count": [
        (
            "a",
            "len(list_filter(CAST(to_json(a) AS JSON[]), lambda e: CAST(e AS VARCHAR) <> 'null'))",
        )
    ],
}
# Declared result types, by shim or by shim and argument count.
RETURNS: dict[str, str] = {
    "sf_strip_null_value": "JSON",
    "sf_try_to_number": "BIGINT",
    "sf_try_to_number_3": "DECIMAL(38, 9)",
    "sf_to_number": "BIGINT",
    "sf_to_number_3": "DECIMAL(38, 9)",
    "sf_get_path": "JSON",
    "sf_get": "JSON",
    "sf_to_timestamp_ntz": "TIMESTAMP",
    "sf_try_to_timestamp_ntz": "TIMESTAMP",
    "sf_try_to_timestamp_tz": "TIMESTAMPTZ",
    "sf_to_timestamp_tz": "TIMESTAMPTZ",
    "sf_object_keys": "VARCHAR[]",
    "sf_is_object": "BOOLEAN",
    "sf_initcap": "VARCHAR",
    "sf_array_size": "BIGINT",
    "sf_regexp_substr": "VARCHAR",
    "sf_to_varchar": "VARCHAR",
    "sf_is_decimal": "BOOLEAN",
    "sf_is_integer": "BOOLEAN",
    "sf_typeof": "VARCHAR",
    "sf_array_contains": "BOOLEAN",
    "sf_array_flatten": "JSON[]",
    "sf_regexp_like": "BOOLEAN",
    "sf_array_distinct": "JSON[]",
    "sf_sha2": "VARCHAR",
    "sf_list_count": "BIGINT",
    "sf_object_agg": "JSON",
    "sf_hash": "BIGINT",
}
# Snowflake functions as Polyglot leaves them, and their shims or DuckDB equivalents.
RENAMES: dict[str, str] = {
    "STRIP_NULL_VALUE": "sf_strip_null_value",
    "TRY_TO_NUMBER": "sf_try_to_number",
    "GET_PATH": "sf_get_path",
    "TO_TIMESTAMP_NTZ": "sf_to_timestamp_ntz",
    "TRY_TO_TIMESTAMP_TZ": "sf_try_to_timestamp_tz",
    "OBJECT_KEYS": "sf_object_keys",
    "IS_OBJECT": "sf_is_object",
    "INITCAP": "sf_initcap",
    "ARRAY_LENGTH": "sf_array_size",
    "ARRAY_SIZE": "sf_array_size",
    "ARRAY_CONSTRUCT": "json_array",
    "OBJECT_CONSTRUCT": "json_object",
    "REGEXP_SUBSTR": "sf_regexp_substr",
    "TO_VARCHAR": "sf_to_varchar",
    "TO_NUMBER": "sf_to_number",
    "PARSE_URL": "sf_parse_url",
    "OBJECT_AGG": "sf_object_agg",
    "HASH": "sf_hash",
    "JSON_GROUP_OBJECT": "sf_object_agg",
    "IS_DECIMAL": "sf_is_decimal",
    "IS_INTEGER": "sf_is_integer",
    "TYPEOF": "sf_typeof",
    "ARRAY_CONTAINS": "sf_array_contains",
    "LIST_COUNT": "sf_list_count",
    "ARRAY_DISTINCT": "sf_array_distinct",
    "LIST_DISTINCT": "sf_array_distinct",
    "SHA2": "sf_sha2",
    "DATEADD": "sf_dateadd",
    "FLATTEN": "sf_array_flatten",
    "TRY_TO_TIMESTAMP_NTZ": "sf_try_to_timestamp_ntz",
    "TO_TIMESTAMP_TZ": "sf_to_timestamp_tz",
    "REGEXP_LIKE": "sf_regexp_like",
}
LIST_FUNCTIONS = {
    "LIST_TRANSFORM": "list_transform",
    "TRANSFORM": "list_transform",
    "LIST_FILTER": "list_filter",
    "FILTER": "list_filter",
    "LIST_MIN": "list_min",
    "LIST_MAX": "list_max",
}
FLATTEN_COLUMNS = re.compile(
    r'\)\s+AS\s+(\w+)\(\s*SEQ,\s*KEY,\s*PATH,\s*"INDEX",\s*VALUE,\s*THIS\s*\)'
)
SNOWFLAKE_FORMATS = [
    ("YYYY", "%Y"),
    ("HH24", "%H"),
    ("MON", "%b"),
    ("MM", "%m"),
    ("DD", "%d"),
    ("DY", "%a"),
    ("MI", "%M"),
    ("SS", "%S"),
]
# Columns that carry JSON values through a CTE, where a text cast must unquote them, by model.
JSON_COLUMNS = {
    "int_product_specs_flat": {"pv.material", "pv.colour_family", "pv.dishwasher_safe"},
    "int_survey_answers_flat": {"t.answer_value"},
}
JSON_FUNCTIONS = ("sf_strip_null_value", "sf_get", "sf_get_path", "to_json")


def balanced(text: str, start: int) -> int:
    """Index just past the parenthesis that closes the one opened before `start`."""

    depth, i, quote = 1, start, None
    while depth:
        c = text[i]
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
        i += 1
    return i


def split_args(text: str) -> list[str]:
    """Top-level comma-separated arguments, keeping a lambda's parameters together."""

    args, depth, quote, start = [], 0, None, 0
    for i, c in enumerate(text):
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == "," and depth == 0:
            args.append(text[start:i])
            start = i + 1
    args.append(text[start:])
    merged: list[str] = []
    for arg in args:
        if merged and re.match(r"\s*LAMBDA\b", merged[-1], re.I) and ":" not in merged[-1]:
            merged[-1] += "," + arg
        else:
            merged.append(arg)
    return merged


def calls(sql: str, name: str, replace: Callable[[list[str]], str]) -> str:
    """Rewrite every call of `name`, innermost first, handing its argument list to `replace`."""

    pattern = re.compile(rf"(?<![\w.]){name}\(", re.I)
    for m in reversed(list(pattern.finditer(sql))):
        end = balanced(sql, m.end())
        sql = sql[: m.start()] + replace(split_args(sql[m.end() : end - 1])) + sql[end:]
    return sql


def flatten(sql: str) -> str:
    """Polyglot's LATERAL UNNEST of a FLATTEN call, as a subquery with FLATTEN's columns."""

    out, i = [], 0
    for m in re.finditer(r"LATERAL UNNEST\(\s*input => ", sql):
        if m.start() < i:
            continue
        end = balanced(sql, m.end())
        args = sql[m.end() : end - 1]
        outer = re.search(r',\s*"outer" => TRUE', args) is not None
        expr = re.sub(r',\s*"outer" => TRUE', "", args)
        alias = FLATTEN_COLUMNS.match(sql, end - 1)
        if not alias:
            continue
        name = alias.group(1)
        nulls = "CAST(NULL AS BIGINT) AS seq"
        if re.search(rf"(?<![\w.]){name}\.key\b", sql):
            # An object's keys: KEY and VALUE per key.
            obj = f"CAST({expr} AS JSON)"
            body = (
                f"(SELECT {nulls}, f.k AS key, CAST(NULL AS VARCHAR) AS path, "
                f'CAST(NULL AS BIGINT) AS "index", {obj} -> f.k AS value, '
                f"CAST(NULL AS JSON) AS this "
                f"FROM (SELECT UNNEST(json_keys({obj})) AS k) AS f) AS {name}"
            )
        else:
            # An array's elements: INDEX and VALUE per element; OUTER keeps one null row.
            items = f"CAST(to_json({expr}) AS JSON[])"
            if outer:
                items = (
                    f"CASE WHEN COALESCE(len({items}), 0) = 0 THEN [CAST(NULL AS JSON)] "
                    f"ELSE {items} END"
                )
            body = (
                f"(SELECT {nulls}, CAST(NULL AS VARCHAR) AS key, CAST(NULL AS VARCHAR) AS path, "
                f'CAST(f.i - 1 AS BIGINT) AS "index", f.e AS value, CAST(NULL AS JSON) AS this '
                f"FROM (SELECT UNNEST({items}) AS e, generate_subscripts({items}, 1) AS i) AS f) "
                f"AS {name}"
            )
        out.append(sql[i : m.start()])
        out.append("LATERAL " + body)
        i = alias.end()
    out.append(sql[i:])
    return "".join(out)


def lambda_arg(arg: str) -> str:
    """Snowflake's `x -> body` lambda as DuckDB's `lambda x: body`. Its list holds JSON, so a text
    cast of the element unquotes it, as Snowflake's ::VARCHAR does."""

    m = re.match(r"^\s*(?:LAMBDA\s+)?(\w+)\s*(?:->|:)", arg, re.I)
    if m:
        element = m.group(1)
        arg = re.sub(
            rf"CAST\({element} AS (TEXT|VARCHAR)\)", f"json_extract_string({element}, '$')", arg
        )
    return re.sub(r"^(\s*)(\w+) -> ", r"\1lambda \2: ", arg)


def json_list(arg: str) -> str:
    arg = arg.strip()
    if arg.startswith(("CAST(to_json(", "list_")):
        return arg
    return f"CAST(to_json({arg}) AS JSON[])"


def casts(sql: str) -> str:
    """CAST(x AS VARIANT|OBJECT|ARRAY) makes a JSON value, as Snowflake makes a variant."""

    i = 0
    pattern = re.compile(r"(?<![\w.])CAST\(", re.I)
    while m := pattern.search(sql, i):
        end = balanced(sql, m.end())
        inner = sql[m.end() : end - 1]
        t = re.search(r"\sAS (VARIANT|OBJECT|ARRAY)\s*$", inner)
        if t:
            value = f"to_json({inner[: t.start()]})"
            # A select item keeps an outer cast, so its declared JSON type is proven.
            if re.match(r"\s+AS\s+\w+\s*(,|\n|$)", sql[end:]):
                value = f"CAST({value} AS JSON)"
            sql = sql[: m.start()] + value + sql[end:]
            i = m.start()
        else:
            i = m.end()
    return sql


def asof(sql: str) -> str:
    """Snowflake's ASOF JOIN ... MATCH_CONDITION (c) ON k as DuckDB's ASOF JOIN ... ON k AND c."""

    while m := re.search(r"\s*MATCH_CONDITION \(", sql):
        end = balanced(sql, m.end())
        condition = sql[m.end() : end - 1]
        rest = sql[end:]
        on = re.match(r"(\s*ON [^\n]+)", rest)
        if on is None:
            raise ValueError("MATCH_CONDITION without an ON clause on the next line")
        sql = sql[: m.start()] + on.group(1) + f" AND {condition}" + rest[on.end() :]
    return sql


def strftime_format(m: re.Match[str]) -> str:
    fmt = m.group(2)
    if "%" not in fmt:
        for old, new in SNOWFLAKE_FORMATS:
            fmt = fmt.replace(old, new)
    return f"{m.group(1)}'{fmt}')"


def json_text(sql: str, model: str | None) -> str:
    """CAST(json AS TEXT) keeps a JSON string's quotes in DuckDB; Snowflake's ::VARCHAR drops
    them, so a text cast of a JSON value reads it as a string."""

    pattern = re.compile(r"(?<![\w.])CAST\(", re.I)
    i = 0
    while m := pattern.search(sql, i):
        end = balanced(sql, m.end())
        inner = sql[m.end() : end - 1]
        t = re.search(r"\sAS (TEXT|VARCHAR)\s*$", inner)
        value = inner[: t.start()].strip() if t else ""
        bare = value
        while bare.startswith("(") and balanced(bare, 1) == len(bare):
            bare = bare[1:-1].strip()
        head = re.match(r"(\w+)\(", bare)
        function = head.group(1).lower() if head else ""
        jsonish = (
            function in JSON_FUNCTIONS
            or bare in JSON_COLUMNS.get(model or "", ())
            or re.fullmatch(r"[\w.]+ -> '[^']*'", bare)
            or re.fullmatch(r"\w+\.value", bare)
            or (head and head.group(1) == "COALESCE" and "to_json(" in bare)
        )
        if t and jsonish:
            text = f"json_extract_string({value}, '$') AS {t.group(1)}"
            sql = sql[: m.end()] + text + sql[end - 1 :]
        i = m.end()
    return sql


def list_call(name: str, target: str) -> Callable[[list[str]], str]:
    def replace(args: list[str]) -> str:
        if args[0].strip().upper().startswith("WHERE"):
            return f"{name}({','.join(args)})"
        return f"{target}({', '.join([json_list(args[0]), *map(lambda_arg, args[1:])])})"

    return replace


def list_functions(sql: str) -> str:
    """Snowflake's array functions on JSON lists, as DuckDB's list functions."""

    for name, target in LIST_FUNCTIONS.items():
        sql = calls(sql, name, list_call(name, target))
    sql = calls(
        sql, "LIST_APPEND", lambda a: f"list_append({json_list(a[0])}, to_json({a[1].strip()}))"
    )
    sql = calls(
        sql, "LIST_PREPEND", lambda a: f"list_prepend(to_json({a[0].strip()}), {json_list(a[1])})"
    )
    sql = calls(sql, "LIST_SORT", lambda a: f"list_sort({', '.join([json_list(a[0]), *a[1:]])})")
    sql = calls(sql, "LIST_CONCAT", lambda a: f"list_concat({', '.join(map(json_list, a))})")
    return calls(
        sql,
        "REDUCE",
        lambda a: f"list_reduce({json_list(a[0])}, {a[2].strip()}, {a[1].strip()})",
    )


def rewrite(sql: str, model: str | None = None) -> str:
    """Finish Polyglot's DuckDB translation of one model, function or expression."""

    sql = flatten(sql)
    # A flattened JSON string read as text loses its quotes, as Snowflake's ::VARCHAR does.
    sql = re.sub(r"CAST\((\w+)\.value AS (TEXT|VARCHAR)\)", r"(\1.value ->> '$')", sql)
    sql = re.sub(
        r"(STRFTIME\((?:[^()]|\([^()]*\))*?, )'([^']*)'\)", strftime_format, sql, flags=re.I
    )
    sql = re.sub(r"(?<![\w.])GET_PATH\(([\w.]+), '([\w.]+)'\)", r"\1 -> '$.\2'", sql, flags=re.I)
    sql = re.sub(r"CAST\((JSON\('[^']*'\))\[([\w.]+)\] AS TEXT\)", r"(\1 ->> \2)", sql)
    sql = re.sub(
        r"(?<![\w.])LISTAGG\(([^,()]+), ('[^']*') ORDER BY ([^()]*)\)",
        r"ARRAY_TO_STRING(ARRAY_AGG(\1 ORDER BY \3), \2)",
        sql,
        flags=re.I,
    )
    sql = re.sub(r"(?<![\w.])CURRENT_TIMESTAMP\(\)", "CURRENT_TIMESTAMP", sql, flags=re.I)
    sql = re.sub(
        r"(?<![\w.])RATIO_TO_REPORT\(([^()]+)\) OVER \(", r"(\1) / SUM(\1) OVER (", sql, flags=re.I
    )
    # Polyglot casts both sides of a sub-day DATEDIFF to DATE, which drops the time of day.
    sql = re.sub(
        r"DATE_DIFF\('(SECOND|MINUTE|HOUR)', CAST\(([^()]+) AS DATE\), CAST\(([^()]+) AS DATE\)\)",
        r"DATE_DIFF('\1', \2, \3)",
        sql,
    )
    sql = asof(sql)
    sql = casts(sql)
    sql = re.sub(r"::(OBJECT|ARRAY|VARIANT)\b", "::JSON", sql)
    sql = re.sub(r",\s*\[\]\)", ", '[]'::JSON)", sql)
    sql = list_functions(sql)
    for name, shim in RENAMES.items():
        sql = re.sub(rf"(?<![\w.]){name}\(", f"{shim}(", sql, flags=re.I)
    sql = re.sub(r"(\d{2}:\d{2}:\d{2}(?:\.\d+)?) ?([+-]\d{2})(\d{2})'", r"\1\2:\3'", sql)
    # A JSON field read as text loses its quotes, as Snowflake's ::VARCHAR does.
    sql = re.sub(r"CAST\(([\w.]+) -> ('[^']*') AS (TEXT|VARCHAR)\)", r"(\1 ->> \2)", sql)
    sql = re.sub(
        r"CAST\(sf_strip_null_value\(([\w.]+) -> ('[^']*')\) AS (TEXT|VARCHAR)\)",
        r"(\1 ->> \2)",
        sql,
    )
    sql = json_text(sql, model)
    arrow = r"[\w.]+ -> '[^']*'"
    return re.sub(
        rf"CAST\(COALESCE\(((?:{arrow}, )+{arrow})\) AS (TEXT|VARCHAR)\)",
        lambda m: "COALESCE(" + m.group(1).replace(" -> ", " ->> ") + ")",
        sql,
    )


def replace_cte(sql: str, name: str, body: str) -> str:
    m = re.search(rf"(?<![\w.]){name} AS \(", sql)
    if m is None:
        raise ValueError(f"no CTE named {name}")
    end = balanced(sql, m.end())
    return sql[: m.end()] + "\n" + body + "\n" + sql[end - 1 :]


def _dim_date(sql: str) -> str:
    return replace_cte(
        sql,
        "generated",
        "  SELECT CAST(d AS DATE) AS calendar_date\n"
        "  FROM range(DATE '2025-01-01', DATE '2025-01-01' + INTERVAL 1095 DAY, INTERVAL 1 DAY)"
        " AS t(d)",
    )


def _category_paths(sql: str) -> str:
    tree = (
        "  SELECT c.category_id, c.parent_category_id, c.slug, c.display_name, c.sort_order,\n"
        "    1 AS depth, '/' || c.slug AS slug_path, c.category_id AS root_category_id\n"
        "  FROM categories AS c\n  WHERE c.is_root\n  UNION ALL\n"
        "  SELECT c.category_id, c.parent_category_id, c.slug, c.display_name, c.sort_order,\n"
        "    t.depth + 1, t.slug_path || '/' || c.slug, t.root_category_id\n"
        "  FROM categories AS c\n  INNER JOIN tree AS t ON c.parent_category_id = t.category_id"
    )
    return re.sub(r"^WITH ", "WITH RECURSIVE ", replace_cte(sql, "tree", tree), count=1)


# Snowflake constructs with no mechanical DuckDB form (GENERATOR, CONNECT BY), rewritten by hand.
OVERRIDES: dict[str, Callable[[str], str]] = {
    "dim_date": _dim_date,
    "int_category_paths": _category_paths,
}


def convert(sql: str, model: str | None = None) -> str:
    sql = rewrite(sql, model)
    return OVERRIDES[model](sql) if model in OVERRIDES else sql


def shim_function_name(name: str, arity: int) -> str:
    return f"{name}_{arity}" if len(SHIMS[name]) > 1 else name


def shim_calls(sql: str) -> str:
    """Calls of shims as calls of their project SQL macros."""

    longest_first = sorted(SHIMS, key=lambda name: len(name), reverse=True)
    for name in longest_first:
        sql = calls(sql, name, shim_call(name))
    return sql


def shim_call(name: str) -> Callable[[list[str]], str]:
    def replace(args: list[str]) -> str:
        return f"@sql.{shim_function_name(name, len(args))}({','.join(args)})"

    return replace


def parameters(body: str, names: list[str]) -> str:
    """Each parameter as `@name`, outside string literals and lambda parameters."""

    parts = re.split(r"('(?:[^']|'')*')", body)
    for index in range(0, len(parts), 2):
        for name in names:
            parts[index] = re.sub(rf"(?<![\w.@']){name}\b(?!\s*:)", f"@{name}", parts[index])
    return "".join(parts)


def macro_folder(root: pathlib.Path, folders: list[pathlib.Path]) -> pathlib.Path:
    """The declaration folder that owns a macro called from `folders`, as SQLBuild requires:
    private to one folder, or public at the folders' closest common folder."""

    if len(folders) == 1:
        return folders[0] / "_sqlbuild" / "_macros" / "sql"
    common = pathlib.Path(os.path.commonpath(folders))
    if common in (root, root / "models"):
        return root / "macros" / "sql"
    return common / "_sqlbuild" / "macros" / "sql"


def write_shims(root: pathlib.Path) -> None:
    """Write each shim overload a model calls, in the folder that owns its callers."""

    callers: dict[str, set[pathlib.Path]] = {}
    for path in root.glob("models/**/*.sql"):
        for name in re.findall(r"@sql\.(\w+)\(", path.read_text()):
            callers.setdefault(name, set()).add(path.parent)
    for name, overloads in SHIMS.items():
        for params, body in overloads:
            names = [p.strip() for p in params.split(",")]
            macro = shim_function_name(name, len(names))
            if macro not in callers:
                continue
            directory = macro_folder(root, sorted(callers[macro]))
            directory.mkdir(parents=True, exist_ok=True)
            returns = RETURNS.get(macro, RETURNS.get(name))
            function = name.removeprefix("sf_").upper()
            arguments = ", ".join(f"{p} ANY" for p in names)
            (directory / f"{macro}.sql").write_text(
                "MACRO (\n"
                f"  description \"Snowflake's {function} for DuckDB, "
                f'taking {len(names)} arguments",\n'
                f"  arguments ({arguments}),\n"
                + (f"  returns {returns},\n" if returns else "")
                + ");\n\n"
                f"{parameters(body, names)}\n"
            )
