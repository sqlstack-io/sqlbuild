TEST (name "udf__order_band__labels_amounts");

WITH input_values AS (
  SELECT
    v.amount
  FROM VALUES
    (5),
    (50) AS v(amount)
),

__udf_actual__ AS (
  SELECT
    i.amount,
    __udf("udf__order_band")(i.amount) AS band
  FROM input_values AS i
),

__udf_expected__ AS (
  SELECT
    v.amount::INT AS amount,
    v.band::VARCHAR AS band
  FROM VALUES
    (5, 'large'),
    (50, 'large') AS v(amount, band)
)
