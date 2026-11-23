MODEL (
  description """
    One row per return with its inspection grade, the refund paid against the return's order, the
    days from request to receipt and to refund, and the final outcome.
  """,
  columns (
    return_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR),
    sku (type VARCHAR),
    return_reason (type VARCHAR),
    grade (type VARCHAR),
    is_resellable (type BOOLEAN),
    refund_usd (type NUMBER(18, 2)),
    days_to_receive (type INT),
    days_to_refund (type INT),
    outcome (type VARCHAR, audits [
      accepted_values (values ["restocked", "written_off", "refunded_not_received", "pending"]),
    ]),
    defect_count (type INT),
  ),
);

WITH returns AS (
  SELECT *
  FROM __ref("int_return_reasons")
),

raw_returns AS (
  SELECT *
  FROM __ref("stg_returns")
),

inspections AS (
  SELECT *
  FROM __ref("stg_return_inspections")
),

refunds AS (
  SELECT *
  FROM __ref("int_refunds_usd")
),

latest_inspection AS (
  SELECT
    i.return_id,
    i.grade,
    i.is_resellable,
    ARRAY_SIZE(i.defect_codes) AS defect_count
  FROM inspections AS i
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY i.return_id
      ORDER BY i.inspected_at_local DESC, i.inspection_id DESC
    ) = 1
),

order_refunds AS (
  SELECT
    f.order_id,
    SUM(f.amount_usd) AS refund_usd,
    MIN(f.refunded_at) AS first_refunded_at
  FROM refunds AS f
  WHERE f.reason_category <> 'fraud'
  GROUP BY
    f.order_id
),

final AS (
  SELECT
    r.return_id,
    r.order_id,
    r.sku,
    r.return_reason,
    li.grade,
    li.is_resellable,
    orf.refund_usd::NUMBER(18, 2) AS refund_usd,
    DATEDIFF('day', rr.requested_at, rr.received_at)::INT AS days_to_receive,
    DATEDIFF('day', rr.requested_at, orf.first_refunded_at)::INT AS days_to_refund,
    CASE
      WHEN li.is_resellable
      THEN 'restocked'
      WHEN li.grade IS NOT NULL
      THEN 'written_off'
      WHEN orf.refund_usd IS NOT NULL
      THEN 'refunded_not_received'
      ELSE 'pending'
    END::VARCHAR AS outcome,
    COALESCE(li.defect_count, 0)::INT AS defect_count
  FROM returns AS r
  INNER JOIN raw_returns AS rr
    ON r.return_id = rr.return_id
  LEFT JOIN latest_inspection AS li
    ON r.return_id = li.return_id
  LEFT JOIN order_refunds AS orf
    ON r.order_id = orf.order_id
)

SELECT *
FROM final
