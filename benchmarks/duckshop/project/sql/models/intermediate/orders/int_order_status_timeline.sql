MODEL (
  description """
    One row per order summarising its status history: when each milestone was first reached, time
    spent in each status, the path taken, and whether the status ever moved backwards.
  """,
  constants (
    _status_rank constant(value (pending 1, paid 2, packed 3, shipped 4, delivered 5, returned 6, cancelled 7)),
  ),
  columns (
    order_id (type VARCHAR, nullable false, audits [not_null, unique]),
    transition_count (type INT),
    first_paid_at (type TIMESTAMP_TZ),
    first_shipped_at (type TIMESTAMP_TZ),
    first_delivered_at (type TIMESTAMP_TZ),
    cancelled_at (type TIMESTAMP_TZ),
    minutes_to_pay (type INT),
    hours_paid_to_shipped (type NUMBER(12, 2)),
    status_path (type VARCHAR),
    has_regression (type BOOLEAN),
    last_status (type VARCHAR),
    longest_status (type VARCHAR),
  ),
);

WITH status_events AS (
  SELECT *
  FROM __ref("stg_order_status_events")
),

orders AS (
  SELECT *
  FROM __ref("stg_orders")
),

ranked_events AS (
  SELECT
    e.event_id,
    e.order_id,
    e.to_status,
    e.changed_at,
    @const("_status_rank")[e.to_status]::INT AS status_rank,
    LEAD(e.changed_at) OVER (
      PARTITION BY e.order_id
      ORDER BY e.changed_at, e.event_id
    ) AS next_changed_at
  FROM status_events AS e
),

with_previous AS (
  SELECT
    r.event_id,
    r.order_id,
    r.to_status,
    r.changed_at,
    r.status_rank,
    r.next_changed_at,
    LAG(r.status_rank) OVER (
      PARTITION BY r.order_id
      ORDER BY r.changed_at, r.event_id
    ) AS previous_rank
  FROM ranked_events AS r
),

durations AS (
  SELECT
    w.order_id,
    w.to_status,
    SUM(
      DATEDIFF('minute', w.changed_at, COALESCE(w.next_changed_at, w.changed_at))
    ) AS minutes_in_status
  FROM with_previous AS w
  GROUP BY
    w.order_id,
    w.to_status
),

longest AS (
  SELECT
    d.order_id,
    MAX_BY(d.to_status, d.minutes_in_status) AS longest_status
  FROM durations AS d
  GROUP BY
    d.order_id
),

milestones AS (
  SELECT
    w.order_id,
    COUNT(*) AS transition_count,
    MIN(IFF(w.to_status = 'paid', w.changed_at, NULL)) AS first_paid_at,
    MIN(IFF(w.to_status = 'shipped', w.changed_at, NULL)) AS first_shipped_at,
    MIN(IFF(w.to_status = 'delivered', w.changed_at, NULL)) AS first_delivered_at,
    MAX(IFF(w.to_status = 'cancelled', w.changed_at, NULL)) AS cancelled_at,
    LISTAGG(w.to_status, ' > ') WITHIN GROUP (ORDER BY w.changed_at, w.event_id) AS status_path,
    BOOLOR_AGG(
      w.status_rank < w.previous_rank AND w.to_status NOT IN ('cancelled', 'returned')
    ) AS has_regression,
    MAX_BY(w.to_status, w.changed_at) AS last_status
  FROM with_previous AS w
  GROUP BY
    w.order_id
),

final AS (
  SELECT
    m.order_id,
    m.transition_count::INT AS transition_count,
    m.first_paid_at,
    m.first_shipped_at,
    m.first_delivered_at,
    m.cancelled_at,
    DATEDIFF('minute', o.placed_at, m.first_paid_at)::INT AS minutes_to_pay,
    ROUND(
      DATEDIFF('minute', m.first_paid_at, m.first_shipped_at) / 60,
      2
    )::NUMBER(12, 2) AS hours_paid_to_shipped,
    m.status_path::VARCHAR AS status_path,
    COALESCE(m.has_regression, FALSE) AS has_regression,
    m.last_status::VARCHAR AS last_status,
    l.longest_status::VARCHAR AS longest_status
  FROM milestones AS m
  INNER JOIN orders AS o
    ON m.order_id = o.order_id
  LEFT JOIN longest AS l
    ON m.order_id = l.order_id
)

SELECT *
FROM final
