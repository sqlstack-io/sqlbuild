-- Published jobs no longer record which layer set their run concurrency: drop the removed
-- resolved_concurrency.source from each job's stored definition, keeping its limit.
UPDATE sqb_jobs SET definition_json = json_remove(definition_json, '$.resolved_concurrency.source') WHERE json_extract(definition_json, '$.resolved_concurrency.source') IS NOT NULL;
