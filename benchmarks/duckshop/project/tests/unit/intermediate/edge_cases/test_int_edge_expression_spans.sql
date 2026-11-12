TEST (
  name "int_edge_expression_spans__converts_prices_loaded_in_the_window",
  cursor_start "2026-03-02 00:00:00",
  cursor_end "2026-03-03 00:00:00",
);

WITH __source__edge__price_windows AS (
  SELECT
    v.window_id,
    v.sku,
    v.price_minor::NUMBER(38, 0) AS price_minor,
    v.valid_from::TIMESTAMP_NTZ(6) AS valid_from,
    v.valid_to::TIMESTAMP_NTZ(6) AS valid_to,
    v._loaded_at::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('PW-01', 'SKU-1', 1234, '2026-01-01 00:00:00', '9999-12-31 00:00:00', '2026-03-02 09:00:00'),
    ('PW-02', 'SKU-2', NULL, '2026-01-01 00:00:00', '9999-12-31 00:00:00', '2026-03-02 18:30:00'),
    ('PW-03', 'SKU-3', 999, '2026-01-01 00:00:00', '9999-12-31 00:00:00', '2026-03-01 23:59:59') AS v(window_id, sku, price_minor, valid_from, valid_to, _loaded_at)
),

__expected__int_edge_expression_spans AS (
  SELECT
    'PW-01' AS window_id,
    12.34::NUMBER(38, 6) AS price_amount,
    12.34::NUMBER(18, 2) AS rounded_price,
    'USD' AS currency,
    'Prix à jour €' AS label,
    '2026-03-02 09:00:00'::TIMESTAMP_NTZ AS loaded_at
  UNION ALL
  SELECT
    'PW-02' AS window_id,
    CAST(NULL AS NUMBER(38, 6)) AS price_amount,
    CAST(NULL AS NUMBER(18, 2)) AS rounded_price,
    'USD' AS currency,
    'Prix à jour €' AS label,
    '2026-03-02 18:30:00'::TIMESTAMP_NTZ AS loaded_at
)
