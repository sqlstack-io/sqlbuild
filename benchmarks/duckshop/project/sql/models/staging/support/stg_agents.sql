MODEL (
  description "One row per live support agent with skills as an array",
  columns (
    agent_id (type VARCHAR, nullable false, audits [not_null, unique]),
    display_name (type VARCHAR),
    email (type VARCHAR),
    team_code (type VARCHAR),
    iana_timezone (type VARCHAR),
    hired_on (type DATE),
    skills (type ARRAY),
    speaks_german (type BOOLEAN),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("support__agents")
),

@sql.cdc_latest(src, src.key := (agent_id))

final AS (
  SELECT
    a.agent_id,
    a.display_name,
    @sql.normalise_email(a.email) AS email,
    UPPER(a.team_code)::VARCHAR AS team_code,
    a.iana_timezone,
    a.hired_on,
    COALESCE(a.skills, ARRAY_CONSTRUCT())::ARRAY AS skills,
    ARRAY_CONTAINS(
      'lang:de'::VARIANT,
      COALESCE(a.skills, ARRAY_CONSTRUCT())
    )::BOOLEAN AS speaks_german,
    @sql.load_metadata(a)
  FROM cdc_latest AS a
)
SELECT *
FROM final
