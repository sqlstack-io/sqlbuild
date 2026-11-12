TEST (
  name "int_edge_reference_qualified_spans__keeps_every_window",
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
    ('PW-02', NULL, NULL, '2026-01-01 00:00:00', '9999-12-31 00:00:00', '2026-03-02 18:30:00') AS v(window_id, sku, price_minor, valid_from, valid_to, _loaded_at)
),

__expected__int_edge_reference_qualified_spans AS (
  SELECT
    'PW-01' AS window_id,
    'SKU-1' AS sku
  UNION ALL
  SELECT
    'PW-02' AS window_id,
    CAST(NULL AS VARCHAR) AS sku
)
