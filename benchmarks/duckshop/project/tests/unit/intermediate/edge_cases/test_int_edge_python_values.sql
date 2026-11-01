TEST (name "int_edge_python_values__hands_python_values_to_variants");

WITH __ref__stg_edge_basket_lines AS (
  SELECT
    v.line_id,
    v.basket_id,
    v.line_no::NUMBER(38, 0) AS line_no,
    v.sku,
    v.tag
  FROM VALUES
    ('BL-01', 'B-1', 1, 'MALLARD-01', 'nothing'),
    ('BL-02', 'B-1', 2, 'TEAL-02', 'zoned_time'),
    ('BL-03', 'B-1', 3, 'EIDER-03', 'naive_time'),
    ('BL-04', 'B-2', 1, 'PINTAIL-04', 'naive_time_fraction'),
    ('BL-05', 'B-2', 2, 'MALLARD-01', 'exponent_decimal'),
    ('BL-06', 'B-3', 1, 'MERGANSER-05', 'plain_decimal'),
    ('BL-07', 'B-3', 2, 'TEAL-02', 'wide_integer'),
    ('BL-08', 'B-4', 1, 'EIDER-03', 'nested'),
    ('BL-09', 'B-4', 2, 'EIDER-03', 'some other kind'),
    ('BL-10', 'B-4', 3, 'EIDER-03', NULL) AS v(line_id, basket_id, line_no, sku, tag)
),

__expected__int_edge_python_values AS (
  SELECT
    NULL AS kind,
    NULL AS payload_is_sql_null,
    NULL AS payload_is_json_null,
    NULL AS payload_kind,
    NULL AS payload_text,
    NULL AS amount_as_float
)
