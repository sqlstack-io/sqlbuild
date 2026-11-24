MODEL (
  description """
    One row per basket line with its SKU read by text functions: STARTSWITH and ENDSWITH, SUBSTRING,
    POSITION written with IN and with a comma, CHARINDEX, CHR, REGEXP_SUBSTR_ALL, ::STRING and CAST
    ... AS STRING, hexadecimal and Base64 round trips with HEX_ENCODE, HEX_DECODE_STRING,
    TRY_HEX_DECODE_STRING and TRY_BASE64_DECODE_BINARY, and a filter on a text literal that carries
    a trailing comment
  """,
  tags [edge_case],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    is_mallard (type BOOLEAN),
    ends_in_one (type BOOLEAN),
    sku_family (type VARCHAR),
    dash_position (type NUMBER(38, 0)),
    dash_position_comma (type NUMBER(38, 0)),
    second_dash_position (type NUMBER(38, 0)),
    sku_with_marker (type VARCHAR),
    sku_digits (type ARRAY),
    line_no_text (type VARCHAR),
    sku_hex (type VARCHAR),
    sku_from_hex (type VARCHAR),
    odd_hex_text (type VARCHAR),
    tag_base64_hex (type VARCHAR),
    is_gift_line (type BOOLEAN),
  ),
);

WITH lines AS (
  SELECT
    l.line_id,
    l.line_no,
    l.sku,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

final AS (
  SELECT
    l.line_id,
    STARTSWITH(l.sku, 'MALLARD')::BOOLEAN AS is_mallard,
    ENDSWITH(l.sku, '1')::BOOLEAN AS ends_in_one,
    SUBSTRING(l.sku, 1, POSITION('-' IN l.sku) - 1)::STRING AS sku_family,
    POSITION('-' IN l.sku)::NUMBER(38, 0) AS dash_position,
    POSITION('-', l.sku)::NUMBER(38, 0) AS dash_position_comma,
    CHARINDEX('-', l.sku || '-x', 3)::NUMBER(38, 0) AS second_dash_position,
    (l.sku || CHR(35))::STRING AS sku_with_marker,
    REGEXP_SUBSTR_ALL(l.sku, '[0-9]')::ARRAY AS sku_digits,
    CAST(l.line_no AS STRING) AS line_no_text,
    HEX_ENCODE(l.sku)::STRING AS sku_hex,
    HEX_DECODE_STRING(HEX_ENCODE(l.sku))::STRING AS sku_from_hex,
    TRY_HEX_DECODE_STRING(l.tag)::STRING AS odd_hex_text,
    HEX_ENCODE(TRY_BASE64_DECODE_BINARY(l.tag))::STRING AS tag_base64_hex,
    (
      l.tag = 'gift' /* the tag gift baskets carry */
    )::BOOLEAN AS is_gift_line
  FROM lines AS l
)

SELECT *
FROM final
