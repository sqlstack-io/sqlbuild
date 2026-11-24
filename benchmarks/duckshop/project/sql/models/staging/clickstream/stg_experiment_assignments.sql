MODEL (
  description "One row per experiment assignment, first assignment per visitor and experiment",
  columns (
    assignment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    experiment_key (type VARCHAR, audits [not_null]),
    variant_key (type VARCHAR, audits [not_null]),
    anonymous_id (type VARCHAR),
    customer_id (type VARCHAR),
    assigned_at (type TIMESTAMP_TZ),
    assignment_surface (type VARCHAR),
    context (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("clickstream__experiment_assignments")
),

final AS (
  SELECT
    a.assignment_id,
    LOWER(a.experiment_key)::VARCHAR AS experiment_key,
    LOWER(a.variant_key)::VARCHAR AS variant_key,
    LOWER(a.anonymous_id)::VARCHAR AS anonymous_id,
    a.customer_id,
    a.assigned_at,
    COALESCE(a.context:surface::VARCHAR, 'web')::VARCHAR AS assignment_surface,
    a.context::OBJECT AS context
  FROM src AS a
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY a.experiment_key, LOWER(a.anonymous_id)
      ORDER BY a.assigned_at, a.assignment_id
    ) = 1
)

SELECT *
FROM final
