CREATE INDEX sqb_node_decisive_history ON recall_events(
    (payload -> 'change' -> 'change' ->> 'node_key'), storage_id DESC
) WHERE producer='scheduler' AND event_type='run_node_finished'
AND (payload -> 'change' -> 'change' ->> 'outcome') IN ('success','failed');
