MODEL (
  description "One row per live marketing campaign with its targeting",
  columns (
    campaign_id (type VARCHAR, nullable false, audits [not_null, unique]),
    campaign_name (type VARCHAR),
    channel (type VARCHAR),
    objective (type VARCHAR),
    starts_on (type DATE),
    ends_on (type DATE),
    budget_minor (type INT),
    currency_code (type VARCHAR),
    target_regions (type ARRAY),
    target_age_min (type INT),
    target_age_max (type INT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("growth__campaigns")
),

@sql.cdc_latest(src, src.key := (campaign_id))

final AS (
  SELECT
    c.campaign_id,
    TRIM(c.campaign_name)::VARCHAR AS campaign_name,
    UPPER(c.channel)::VARCHAR AS channel,
    LOWER(c.objective)::VARCHAR AS objective,
    c.starts_on,
    c.ends_on,
    c.budget_minor::INT AS budget_minor,
    UPPER(c.currency_code)::VARCHAR AS currency_code,
    COALESCE(c.targeting:regions, ARRAY_CONSTRUCT())::ARRAY AS target_regions,
    TRY_TO_NUMBER(SPLIT_PART(
      c.targeting:age_band::VARCHAR,
      '-',
      1
    ))::INT AS target_age_min,
    TRY_TO_NUMBER(SPLIT_PART(
      c.targeting:age_band::VARCHAR,
      '-',
      2
    ))::INT AS target_age_max,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
)
SELECT *
FROM final
