ALTER TABLE sqb_security_audit ADD COLUMN storage_id BIGINT;
WITH ordered AS (
    SELECT event_id, ROW_NUMBER() OVER (ORDER BY occurred_at,event_id) AS storage_id
    FROM sqb_security_audit
)
UPDATE sqb_security_audit SET storage_id=(
    SELECT ordered.storage_id FROM ordered WHERE ordered.event_id=sqb_security_audit.event_id
);
CREATE UNIQUE INDEX sqb_security_audit_storage_id ON sqb_security_audit(storage_id);
