-- Published jobs no longer record which layer set their run concurrency: drop the removed
-- resolved_concurrency.source from each job's stored definition, keeping its limit.
UPDATE sqb_jobs SET definition_json = definition_json #- ARRAY['resolved_concurrency', 'source'] WHERE definition_json -> 'resolved_concurrency' -> 'source' IS NOT NULL;
