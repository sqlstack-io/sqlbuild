-- The index the Overview's attention acknowledgement listing reads: each item's latest
-- acknowledgement or withdrawal. IF NOT EXISTS skips an index an operator pre-built.
CREATE INDEX IF NOT EXISTS sqb_security_audit_attention ON sqb_security_audit((scope_json ->> 'location_id'), (scope_json ->> 'target'), (scope_json ->> 'key'), storage_id) WHERE action IN ('attention.acknowledged', 'attention.acknowledgement_withdrawn');
