MODEL (
  description """
    One row per experiment variant with exposure and conversion counts, conversion rate, absolute
    lift over the control and a two-proportion z score.
  """,
  columns (
    experiment_variant_key (type VARCHAR, nullable false, audits [not_null, unique]),
    experiment_key (type VARCHAR),
    variant_key (type VARCHAR),
    is_control (type BOOLEAN),
    exposed (type INT),
    converted (type INT),
    conversion_rate (type NUMBER(9, 6)),
    lift_vs_control (type NUMBER(9, 6)),
    z_score (type NUMBER(9, 4)),
    median_days_to_convert (type NUMBER(10, 2)),
  ),
);

WITH exposures AS (
  SELECT *
  FROM __ref("int_experiment_exposures")
),

variants AS (
  SELECT
    e.experiment_key,
    e.variant_key,
    BOOLOR_AGG(e.is_control) AS is_control,
    COUNT(*) AS exposed,
    COUNT_IF(e.converted) AS converted,
    MEDIAN(e.days_to_convert) AS median_days_to_convert
  FROM exposures AS e
  GROUP BY
    e.experiment_key,
    e.variant_key
),

controls AS (
  SELECT
    v.experiment_key,
    SUM(v.exposed) AS control_exposed,
    SUM(v.converted) AS control_converted
  FROM variants AS v
  WHERE v.is_control
  GROUP BY
    v.experiment_key
),

compared AS (
  SELECT
    v.experiment_key,
    v.variant_key,
    v.is_control,
    v.exposed,
    v.converted,
    v.median_days_to_convert,
    v.converted / NULLIF(v.exposed, 0) AS rate,
    c.control_converted / NULLIF(c.control_exposed, 0) AS control_rate,
    (
      v.converted + c.control_converted
    )
    / NULLIF(v.exposed + c.control_exposed, 0) AS pooled_rate,
    c.control_exposed
  FROM variants AS v
  LEFT JOIN controls AS c
    ON v.experiment_key = c.experiment_key
),

final AS (
  SELECT
    (
      c.experiment_key || ':' || c.variant_key
    )::VARCHAR AS experiment_variant_key,
    c.experiment_key,
    c.variant_key,
    c.is_control::BOOLEAN AS is_control,
    c.exposed::INT AS exposed,
    c.converted::INT AS converted,
    ROUND(c.rate, 6)::NUMBER(9, 6) AS conversion_rate,
    ROUND(c.rate - c.control_rate, 6)::NUMBER(9, 6) AS lift_vs_control,
    ROUND(
      (
        c.rate - c.control_rate
      )
      / NULLIF(
        SQRT(c.pooled_rate
        * (
          1 - c.pooled_rate
        )
        * (
          1 / NULLIF(c.exposed, 0) + 1 / NULLIF(c.control_exposed, 0)
        )),
        0
      ),
      4
    )::NUMBER(9, 4) AS z_score,
    ROUND(c.median_days_to_convert, 2)::NUMBER(10, 2) AS median_days_to_convert
  FROM compared AS c
)

SELECT *
FROM final
