MODEL (
  description """
    One row per delivered shipment version, rebuilt by delivery date while its input is filtered by
    order date. Disabled: a rerun of a window inserts deliveries that fall outside the deleted
    window again, so they duplicate. It is the acceptance case for guarding out-of-window rows in
    cursor delete_insert models; see RUNBOOK.md, "Pending acceptance cases".
  """,
  tags [edge_case],
  enabled false,
  materialized incremental,
  incremental_strategy delete_insert,
  cursor delivered_on,
  cursor_type timestamp,
  cursor_grain day,
  cursor_inputs (
    edge__delivery_updates ordered_on,
  ),
  columns (
    delivery_id (type VARCHAR, nullable false, audits [not_null, unique]),
    shipment_id (type VARCHAR),
    ordered_on (type DATE),
    delivered_on (type DATE),
  ),
);

WITH updates AS (
  SELECT
    u.delivery_id,
    u.shipment_id,
    u.ordered_on,
    u.delivered_on
  FROM __source("edge__delivery_updates") AS u
),

final AS (
  SELECT
    u.delivery_id,
    u.shipment_id,
    u.ordered_on,
    u.delivered_on
  FROM updates AS u
  WHERE
    u.delivered_on IS NOT NULL
    AND u.ordered_on >= __cursor_start()
    AND u.ordered_on < __cursor_end()
)

SELECT *
FROM final
