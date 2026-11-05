MODEL (description 'Retry a failed cast after warehouse input changes.', materialized table);
WITH source(value) AS (VALUES ('invalid'))
SELECT CASE
    WHEN EXISTS (
        SELECT 1 FROM information_schema.tables WHERE table_name = 'retry_ready'
    ) THEN 17
    ELSE CAST(value AS INTEGER)
END AS amount
FROM source
