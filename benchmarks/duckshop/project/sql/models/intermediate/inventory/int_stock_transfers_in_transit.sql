MODEL (
  description """
    One row per stock transfer with its state and the expected arrival from the lane's historical
    median transit time
  """,
  columns (
    transfer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR),
    from_warehouse_code (type VARCHAR),
    to_warehouse_code (type VARCHAR),
    qty (type INT),
    transfer_state (type VARCHAR, audits [
      accepted_values (values ["planned", "in_transit", "arrived"]),
    ]),
    shipped_at (type TIMESTAMP_TZ),
    expected_arrival_at (type TIMESTAMP_TZ),
    lane_median_transit_hours (type NUMBER(10, 2)),
    is_overdue (type BOOLEAN),
  ),
);

WITH transfers AS (
  SELECT *
  FROM __ref("stg_stock_transfers")
),

lane_history AS (
  SELECT
    t.from_warehouse_code,
    t.to_warehouse_code,
    MEDIAN(t.transit_hours) AS lane_median_transit_hours
  FROM transfers AS t
  WHERE t.arrived_at IS NOT NULL
  GROUP BY
    t.from_warehouse_code,
    t.to_warehouse_code
),

final AS (
  SELECT
    t.transfer_id,
    t.sku,
    t.from_warehouse_code,
    t.to_warehouse_code,
    t.qty,
    CASE
      WHEN t.arrived_at IS NOT NULL
      THEN 'arrived'
      WHEN t.shipped_at IS NOT NULL
      THEN 'in_transit'
      ELSE 'planned'
    END::VARCHAR AS transfer_state,
    t.shipped_at,
    TIMESTAMPADD(
      'minute',
      ROUND(COALESCE(h.lane_median_transit_hours, 48) * 60)::INT,
      t.shipped_at
    )::TIMESTAMP_TZ AS expected_arrival_at,
    h.lane_median_transit_hours::NUMBER(10, 2) AS lane_median_transit_hours,
    (
      t.arrived_at IS NULL
      AND TIMESTAMPADD(
        'minute',
        ROUND(COALESCE(h.lane_median_transit_hours, 48) * 60)::INT,
        t.shipped_at
      ) < '@@valuation_date'::TIMESTAMP_TZ
    ) AS is_overdue
  FROM transfers AS t
  LEFT JOIN lane_history AS h
    ON t.from_warehouse_code = h.from_warehouse_code
    AND t.to_warehouse_code = h.to_warehouse_code
)

SELECT *
FROM final
