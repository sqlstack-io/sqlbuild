MODEL (
  description """
    One row per partner attribute value with VARIANT orderings and array reads: ARRAY_MAX, ARRAY_MIN
    and ARRAY_REVERSE over arrays mixing numbers, strings, booleans, JSON nulls and integers past
    2^53; the largest value of each group by window MAX, across kinds and non-ASCII strings;
    ARRAY_REMOVE of a JSON null; JSON nulls in ARRAY_DISTINCT and ARRAYS_OVERLAP; and the value
    looked up in an array of decimals written with trailing zeros
  """,
  tags [edge_case],
  columns (
    value_id (type VARCHAR, nullable false, audits [not_null, unique]),
    largest_element (type VARIANT),
    smallest_element (type VARIANT),
    reversed_elements (type ARRAY),
    group_largest (type VARIANT),
    without_json_nulls (type ARRAY),
    distinct_element_count (type NUMBER(38, 0)),
    overlaps_json_null (type BOOLEAN),
    in_decimal_array (type BOOLEAN),
    decimal_array_position (type NUMBER(38, 0)),
  ),
);

WITH attribute_values AS (
  SELECT
    v.value_id,
    v.group_code,
    v.attribute_value,
    AS_ARRAY(v.attribute_value) AS elements
  FROM __ref("stg_edge_variant_values") AS v
),

final AS (
  SELECT
    a.value_id,
    ARRAY_MAX(a.elements)::VARIANT AS largest_element,
    ARRAY_MIN(a.elements)::VARIANT AS smallest_element,
    ARRAY_REVERSE(a.elements)::ARRAY AS reversed_elements,
    MAX(a.attribute_value) OVER (PARTITION BY a.group_code) AS group_largest,
    ARRAY_REMOVE(a.elements, PARSE_JSON('null'))::ARRAY AS without_json_nulls,
    ARRAY_SIZE(ARRAY_DISTINCT(a.elements))::NUMBER(38, 0) AS distinct_element_count,
    ARRAYS_OVERLAP(a.elements, ARRAY_CONSTRUCT(PARSE_JSON('null')))::BOOLEAN AS overlaps_json_null,
    ARRAY_CONTAINS(
      a.attribute_value,
      ARRAY_CONSTRUCT(-7.00, 1.00, 'a')
    )::BOOLEAN AS in_decimal_array,
    ARRAY_POSITION(
      a.attribute_value,
      ARRAY_CONSTRUCT(1.50, 1.00, 0.00)
    )::NUMBER(38, 0) AS decimal_array_position
  FROM attribute_values AS a
)

SELECT *
FROM final
