// Generated from sqb server --openapi. Run npm run generate; do not edit.
export interface paths {
    "/api/admin/people/{principal_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changeAdministrator"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/roles": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["assignBuiltinRole"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/logout": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["logoutSession"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/oidc/callback": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Validates one browser-bound code and issues a bounded browser session for an explicitly bound person; no provisioning, personal API tokens or implicit grants. */
        get: operations["oidcIdentity"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/oidc/login": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Begins a browser-bound code flow with S256 PKCE; no account provisioning or grants. */
        get: operations["loginOidc"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/password": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changePassword"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/password/login": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["loginPassword"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/proxy": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Requires a configured socket peer and upstream-replaced X-SQB-User subject; issues a bounded browser session without provisioning, personal API tokens or implicit grants. */
        get: operations["proxyIdentity"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/session": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["currentSession"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/audit": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readSecurityAudit"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/catalog": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonCatalog"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonRuns"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/runs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestJobRun"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/runs/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancelPersonRun"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/runs/rerun": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["rerunPersonRun"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/triggers": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["changeTrigger"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["read_location_health"];
        put?: never;
        post: operations["report_location_health"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/changes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["subscribe"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/audit-results": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_audit_results"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/catalog": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_catalog"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/fingerprints": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_fingerprints"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/janitor-events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_janitor_events"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/microbatches": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_microbatches"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/migrations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_migrations"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/old-name-views": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_old_name_views"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/source-freshness": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_source_freshness"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/watermark": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["watermark"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/runs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_runs"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/runs/{run_id}/invocations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["invocations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/runs/{run_id}/nodes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["nodes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/runs/{run_id}/recall/{view}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["read_run_recall"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/snapshots": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["current"];
        put?: never;
        post: operations["publish"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/snapshots/metadata": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["metadata"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/triggers": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_triggers"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["events"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["ticks"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/work/poll": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["poll"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/work/{request_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["result"];
        put?: never;
        post: operations["complete"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/runs/{run_id}/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_run_event"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/runs/{run_id}/execution": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["execution"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/runs/{run_id}/heartbeat": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["heartbeat"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/runs/{run_id}/output": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["output"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/runs/{run_id}/recall": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["recall"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /**
         * @description Administrator membership and person eligibility share the final-admin invariant.
         * @enum {string}
         */
        AdministratorChange: "grant" | "revoke" | "disable" | "enable";
        ApiErrorBody: {
            error: string;
        };
        AuditResultsBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation?: null | components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseAuditRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        AuthenticationDenial: {
            /** Format: uuid */
            decision_id: string;
            error: string;
        };
        /** @enum {string} */
        BuiltinRole: "viewer" | "operator" | "developer" | "admin";
        /** @description Browser cancellation cannot supply an actor, claim or lifecycle transition. */
        CancelRunRequest: {
            /** Format: uuid */
            event_id: string;
            reason: string;
            /** Format: uuid */
            run_id: string;
        };
        CatalogBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation: components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseCatalogRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        CatalogObservation: {
            /** Format: uuid */
            event_id: string;
            /** Format: date-time */
            observed_at: string;
            record: components["schemas"]["WarehouseCatalogRecord"];
        };
        CatalogPage: {
            next_after?: null | components["schemas"]["WarehouseReconciliationCursor"];
            records: components["schemas"]["CatalogObservation"][];
        };
        CatchUp: "latest" | {
            /** @description Process the oldest bounded batch; later due times remain due for the next transaction. */
            all: {
                max_runs: number;
            };
        };
        /** @description SSE invalidation topics; `all` means refetch the authorized REST resources. */
        ChangeNotice: {
            topic: string;
        };
        /** @description A row whose key was already stored with different content; the first row is kept. */
        ConflictEvidence: {
            existing_sha256: string;
            record_key: string;
            rejected_sha256: string;
            table: components["schemas"]["Table"];
        };
        /** @description The current successful snapshot, read with a consistent generation and change set. */
        CurrentSnapshot: {
            changes: components["schemas"]["SnapshotChange"][];
            documents: {
                [key: string]: unknown;
            };
            nodes: {
                [key: string]: components["schemas"]["SnapshotNode"];
            };
            record: components["schemas"]["SnapshotRecord"];
        };
        /** @description Location-owned Python returns data; only the fenced scheduler may apply it. */
        CustomSensorOutcome: {
            cursor?: string | null;
            run_keys: string[];
            /** @enum {string} */
            status: "completed";
        } | {
            cursor?: string | null;
            reason: string;
            /** @enum {string} */
            status: "skipped";
        } | {
            /** @enum {string} */
            status: "failed";
        } | {
            /** @enum {string} */
            status: "timed_out";
        };
        ExternalIdentityResponse: {
            csrf_token: string;
            /** Format: date-time */
            expires_at: string;
            /** Format: uuid */
            principal_id: string;
            /** Format: uuid */
            session_id: string;
        };
        /** @description Fingerprint observations from one registered reader, independent of automatic polling. */
        FingerprintBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation?: null | components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseFingerprintRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        HeartbeatResponse: {
            cancel_requested: boolean;
        };
        /** @description What one insert batch did, per table. */
        InsertOutcome: {
            conflicts: components["schemas"]["ConflictEvidence"][];
            /** @description Rows written or already present with equal content, by table. */
            delivered: [
                "invocations" | "output" | "events" | "diagnostics" | "artifacts" | "artifact_refs" | "heartbeats" | "delivery" | "meta" | "conflicts",
                number
            ][];
            /** @description Tables whose insert failed, so their rows were not written, and the first failure. */
            failed: components["schemas"]["Table"][];
            failure?: string | null;
        };
        JanitorEventsBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation?: null | components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseJanitorRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        /** @description A trusted operator names a published job; execution inputs are never supplied by the caller. */
        JobRunRequest: {
            job_key: string;
            /** Format: uuid */
            location_id: string;
            /** Format: int64 */
            priority: number;
            reason: string;
            /** Format: uuid */
            run_id: string;
            target: string;
            window?: null | components["schemas"]["RunWindow"];
        };
        /** @description Locations report bounded facts, never connection strings or user exception text. */
        LocationHealth: {
            /** @enum {string} */
            kind: "heartbeat";
        } | {
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "compile_failed";
        } | {
            /** Format: int64 */
            generation: number;
            healthy: boolean;
            /** @enum {string} */
            kind: "connection";
        } | {
            /** Format: int64 */
            elapsed_millis: number;
            failure?: null | components["schemas"]["WarehouseSyncFailure"];
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "warehouse_sync";
            location: components["schemas"]["WarehouseStateLocation"];
            /** Format: int64 */
            records_sent: number;
            table: components["schemas"]["WarehouseHistoryKind"];
        };
        /** @description Receipt time belongs to the server; consumers derive freshness rather than storing availability. */
        LocationHealthObservation: {
            /** Format: date-time */
            received_at: string;
            report: components["schemas"]["LocationHealthReport"];
        };
        LocationHealthReport: {
            /** Format: uuid */
            event_id: string;
            health: components["schemas"]["LocationHealth"];
            /** Format: uuid */
            session_id: string;
            target: string;
        };
        /** @description Only named location operations cross this boundary, never arbitrary SQL or Python. */
        LocationWork: {
            /** @enum {string} */
            kind: "check_connection";
        } | {
            evaluation: Record<string, never>;
            /** @enum {string} */
            kind: "evaluate_sensor";
        };
        LocationWorkClaim: {
            /** Format: uuid */
            claim_id: string;
            /** Format: int64 */
            generation: number;
            /** Format: date-time */
            lease_expires_at: string;
            request: components["schemas"]["LocationWorkRequest"];
            /** Format: uuid */
            session_id: string;
        };
        LocationWorkOutcome: {
            healthy: boolean;
            /** @enum {string} */
            kind: "connection";
        } | {
            /** @enum {string} */
            kind: "sensor_evaluation";
            outcome: components["schemas"]["CustomSensorOutcome"];
        } | {
            /** @enum {string} */
            kind: "superseded";
        };
        LocationWorkPoll: {
            /** Format: uuid */
            session_id: string;
            target: string;
        };
        /** @description Expiry is derived from the deadline; only requests, leases and completed results are stored. */
        LocationWorkRecord: {
            /** Format: date-time */
            completed_at?: string | null;
            /** Format: date-time */
            expires_at: string;
            /** Format: int64 */
            generation: number;
            request_json: components["schemas"]["LocationWorkRequest"];
            /** Format: date-time */
            requested_at: string;
            requested_by: string;
            result_json?: null | components["schemas"]["LocationWorkResult"];
        };
        LocationWorkRequest: {
            /** Format: uuid */
            location_id: string;
            reason: string;
            /** Format: uuid */
            request_id: string;
            target: string;
            work: components["schemas"]["LocationWork"];
        };
        LocationWorkResult: {
            /** Format: uuid */
            claim_id: string;
            outcome: components["schemas"]["LocationWorkOutcome"];
            /** Format: uuid */
            session_id: string;
        };
        MicrobatchesBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation?: null | components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseMicrobatchRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        MigrationsBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation?: null | components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseMigrationRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        MutationResponse: {
            applied: boolean;
        };
        /** @enum {string} */
        NodeEventKind: "success" | "failed" | "recovered";
        /** @enum {string} */
        NodeOutcome: "success" | "warn" | "failed" | "skipped" | "cancelled";
        OldNameViewsBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation?: null | components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseOldNameRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        /** @description Output of one stream that was not recorded because it exceeded `max_output_bytes`. */
        OmittedOutput: {
            /** Format: int64 */
            bytes: number;
            /**
             * Format: int64
             * @description Sequences this gap accounts for, its own included.
             */
            sequences: number;
        };
        /** @description One exact-byte output chunk. */
        OutputChunk: {
            /**
             * Format: int64
             * @description Bytes of the same stream before this chunk.
             */
            byte_offset: number;
            data: number[];
            /**
             * Format: int64
             * @description Output records the queue dropped just before this one.
             */
            dropped_before: number;
            occurred_at: string;
            omitted?: null | components["schemas"]["OmittedOutput"];
            /**
             * Format: int64
             * @description Per invocation, from 0, shared by both streams.
             */
            sequence: number;
            stream: components["schemas"]["OutputStream"];
        };
        /** @description Differing retries retain conflict evidence while preserving the first output bytes. */
        OutputPublication: {
            /** Format: int32 */
            conflicts: number;
            /** Format: int32 */
            duplicates: number;
            /** Format: int32 */
            inserted: number;
        };
        /**
         * @description A process output stream.
         * @enum {string}
         */
        OutputStream: "stdout" | "stderr";
        /** @enum {string} */
        Overlap: "coalesce" | "queue" | "allow";
        PasswordChangeRequest: {
            current_password: string;
            new_password: string;
        };
        PasswordLoginRequest: {
            password: string;
            username: string;
        };
        PasswordSessionResponse: {
            csrf_token: string;
            identity: components["schemas"]["SessionIdentity"];
        };
        /** @enum {string} */
        PlatformEventKind: "heartbeat" | "scheduler_no_leader" | "tick_overdue" | "calendar_horizon_low" | "location_broken" | "launch_failed";
        /** @description One invocation's records; the server owns its claim namespace, start time and version. */
        RecallBatch: {
            invocation_id: string;
            records: components["schemas"]["RecallRecord"][];
        };
        RecallRecord: {
            table: components["schemas"]["Table"];
            values: {
                [key: string]: components["schemas"]["RecallValue"];
            };
        };
        /** @description One stored value. */
        RecallValue: {
            /** @enum {string} */
            kind: "null";
        } | {
            /** @enum {string} */
            kind: "integer";
            /** Format: int64 */
            value: number;
        } | {
            /** @enum {string} */
            kind: "real";
            /** Format: double */
            value: number;
        } | {
            /** @enum {string} */
            kind: "text";
            value: string;
        } | {
            /** @enum {string} */
            kind: "blob";
            value: number[];
        };
        /** @description A retry derives its execution scope from a terminal run, never from caller selectors. */
        RerunRequest: {
            from_failure: boolean;
            /** Format: int64 */
            priority: number;
            reason: string;
            /** Format: uuid */
            run_id: string;
            /** Format: uuid */
            source_run_id: string;
        };
        RoleAssignmentRequest: {
            /** Format: uuid */
            location_id?: string | null;
            /** Format: uuid */
            principal_id: string;
            revoke?: boolean;
            role: components["schemas"]["BuiltinRole"];
            target?: string | null;
        };
        /** @description This is a server-owned instruction, not an arbitrary status supplied by a worker. */
        RunChange: {
            container_id?: string | null;
            /** @enum {string} */
            kind: "launched";
        } | {
            code: string;
            /** @enum {string} */
            kind: "launch_failed";
        } | {
            /** @enum {string} */
            kind: "started";
        } | {
            /** @enum {string} */
            kind: "node_started";
            node_key: string;
        } | {
            error_code?: string | null;
            /** @enum {string} */
            kind: "node_finished";
            node_key: string;
            outcome: components["schemas"]["NodeOutcome"];
            /** Format: int64 */
            rows_affected?: number | null;
            skipped_because_of?: string | null;
        } | {
            /** @enum {string} */
            kind: "unresponsive";
        } | {
            /** @enum {string} */
            kind: "recovered";
        } | {
            actor: string;
            /** @enum {string} */
            kind: "cancel_requested";
            reason?: string;
        } | {
            /** Format: int64 */
            exit_code?: number | null;
            /** @enum {string} */
            kind: "finished";
            outcome: components["schemas"]["RunOutcome"];
            reason: string;
        };
        RunChangeRequest: {
            change: components["schemas"]["RunChange"];
            /** Format: uuid */
            claim_id?: string | null;
            /** Format: uuid */
            event_id: string;
            /** Format: uuid */
            run_id: string;
        };
        /** @description The database owns the claim before a launcher sees the pinned request. */
        RunClaim: {
            /** Format: uuid */
            claim_id: string;
            request: components["schemas"]["RunRequest"];
        };
        /** @enum {string} */
        RunEventKind: "completed" | "success" | "warn" | "partial" | "failed" | "cancelled" | "presumed_failed";
        /** @description Workers compile locally and compare against this immutable snapshot, not the current deploy. */
        RunExecution: {
            claim: components["schemas"]["RunClaim"];
            environment_hash: string;
            node_hashes: {
                [key: string]: string;
            };
            /** @description Derived from completed sibling facts and pinned lineage, never a mutable request rewrite. */
            skipped_nodes?: {
                [key: string]: string;
            };
            sqb_version: string;
        };
        /** @enum {string} */
        RunKind: "job" | "group" | "adhoc";
        RunNodeRecord: {
            /** Format: int64 */
            attempt: number;
            /** Format: date-time */
            ended_at?: string | null;
            error_code?: string | null;
            node_key: string;
            /** Format: int64 */
            rows_affected?: number | null;
            skipped_because_of?: string | null;
            /** Format: date-time */
            started_at?: string | null;
            status: string;
        };
        /** @enum {string} */
        RunOrigin: "manual" | "schedule" | "sensor" | "rerun" | "backfill";
        /**
         * @description Terminal outcomes cannot be confused with a request to change an active status.
         * @enum {string}
         */
        RunOutcome: "success" | "warn" | "partial" | "failed" | "cancelled" | "presumed_failed";
        RunRecord: {
            /** Format: date-time */
            ended_at?: string | null;
            job_key?: string | null;
            /** Format: int64 */
            last_event_storage_id: number;
            /** Format: date-time */
            launched_at?: string | null;
            /** Format: uuid */
            location_id: string;
            node_counts_json: {
                [key: string]: number;
            };
            /** Format: date-time */
            queued_at: string;
            /** Format: uuid */
            run_id: string;
            selection_json: string[];
            /** Format: uuid */
            snapshot_id: string;
            /** Format: date-time */
            started_at?: string | null;
            status: components["schemas"]["RunStatus"];
            target: string;
        };
        /** @description The authorized selection and pinned execution inputs are immutable once requested. */
        RunRequest: {
            concurrency_groups: string[];
            image_id?: string | null;
            job_key?: string | null;
            kind: components["schemas"]["RunKind"];
            /** Format: uuid */
            location_id: string;
            origin: components["schemas"]["RunOrigin"];
            overlap: components["schemas"]["Overlap"];
            /** Format: uuid */
            parent_run_id?: string | null;
            /** Format: int64 */
            priority: number;
            requested_by: string;
            /** Format: uuid */
            rerun_of_run_id?: string | null;
            /** Format: uuid */
            root_run_id: string;
            /** Format: uuid */
            run_id: string;
            selection: string[];
            settings: {
                [key: string]: unknown;
            };
            /** Format: uuid */
            snapshot_id: string;
            target: string;
            /** Format: uuid */
            tick_id?: string | null;
            /** Format: uuid */
            trigger_id?: string | null;
            window?: null | components["schemas"]["RunWindow"];
            write_set: string[];
        };
        /** @enum {string} */
        RunStatus: "queued" | "launched" | "running" | "unresponsive" | "success" | "warn" | "partial" | "failed" | "cancelled" | "presumed_failed";
        /** @description Inclusive typed CLI cursor bounds; the native planner derives exclusive execution ends. */
        RunWindow: {
            /** Format: date-time */
            end: string;
            /** @enum {string} */
            kind: "timestamp";
            /** Format: date-time */
            start: string;
        } | {
            /** Format: int64 */
            end: number;
            /** @enum {string} */
            kind: "integer";
            /** Format: int64 */
            start: number;
        };
        Schedule: {
            expression: string;
            /** @enum {string} */
            kind: "cron";
            timezone: string;
        } | {
            /** @enum {string} */
            kind: "calendar";
            /** Format: date-time */
            refresh_at: string;
            times: string[];
        };
        /** @description Durable security facts with an exclusive storage-order cursor. */
        SecurityAuditEvent: {
            action: string;
            actor: string;
            after_json: unknown;
            before_json: unknown;
            /** Format: uuid */
            event_id: string;
            granting_role: string;
            ip_address?: string | null;
            /** Format: date-time */
            occurred_at: string;
            /** Format: uuid */
            request_id: string;
            scope_json: unknown;
            /** Format: int64 */
            storage_id: number;
        };
        SensorCondition: {
            /** @enum {string} */
            kind: "platform_event";
            kinds: components["schemas"]["PlatformEventKind"][];
        } | {
            executed_successfully?: string[] | null;
            job_key: string;
            /** @enum {string} */
            kind: "run_event";
            kinds: components["schemas"]["RunEventKind"][];
        } | {
            /** @enum {string} */
            kind: "node_event";
            kinds: components["schemas"]["NodeEventKind"][];
            selection: string[];
        } | {
            /** @enum {string} */
            kind: "source_new_data";
            selection: string[];
        } | {
            /** Format: int32 */
            after_seconds: number;
            /** @enum {string} */
            kind: "source_stale";
            selection: string[];
        } | {
            code_hash: string;
            function: string;
            /** @enum {string} */
            kind: "custom";
        };
        SensorDefinition: {
            default_status?: components["schemas"]["SensorStatus"];
            description: string;
            job_key: string;
            /** Format: int32 */
            minimum_interval_seconds: number;
            when: components["schemas"]["SensorCondition"];
        };
        /** @enum {string} */
        SensorStatus: "running" | "stopped";
        SessionIdentity: {
            /** Format: date-time */
            expires_at: string;
            /** Format: uuid */
            principal_id: string;
            /** Format: uuid */
            session_id: string;
        };
        /** @description Node-level changes do not duplicate unchanged definitions or source text. */
        SnapshotChange: {
            after_hash?: string | null;
            before_hash?: string | null;
            change_kind: string;
            subject_key: string;
        };
        /** @description A location's successful compile for one target. */
        SnapshotInput: {
            documents: {
                [key: string]: unknown;
            };
            environment_hash: string;
            /** Format: int64 */
            generation: number;
            git_author?: string | null;
            git_dirty: boolean;
            git_revision?: string | null;
            image_id?: string | null;
            /** Format: uuid */
            location_id: string;
            nodes: {
                [key: string]: components["schemas"]["SnapshotNode"];
            };
            sqb_version: string;
            target: string;
        };
        /** @description A compiled node and its display-only source. SQL and Python are never run by the server. */
        SnapshotNode: {
            compiled_sql?: string | null;
            definition: unknown;
            kind: string;
            python_source?: string | null;
            raw_sql?: string | null;
        };
        /** @description Immutable publication metadata. Generations increase independently per location and target. */
        SnapshotRecord: {
            /** Format: int64 */
            generation: number;
            /** Format: uuid */
            location_id: string;
            manifest_digest: string;
            /** Format: date-time */
            published_at: string;
            /** Format: uuid */
            snapshot_id: string;
            target: string;
        };
        /** @description A bounded page from one registered reader of a published location/target generation. */
        SourceFreshnessBatch: {
            /** Format: int64 */
            generation: number;
            reconciliation?: null | components["schemas"]["WarehouseReconciliationPage"];
            records: components["schemas"]["WarehouseFreshnessRecord"][];
            /** Format: uuid */
            session_id: string;
        };
        /**
         * @description The tables Recall writes.
         * @enum {string}
         */
        Table: "invocations" | "output" | "events" | "diagnostics" | "artifacts" | "artifact_refs" | "heartbeats" | "delivery" | "meta" | "conflicts";
        /** @enum {string} */
        TickOutcome: "requested" | "skipped" | "covered" | "failed" | "deferred";
        TickRecord: {
            /** Format: uuid */
            covered_by_run_id?: string | null;
            /** Format: int64 */
            epoch: number;
            /** Format: date-time */
            evaluated_at: string;
            outcome: components["schemas"]["TickOutcome"];
            reason?: string | null;
            /** Format: uuid */
            run_id?: string | null;
            /** Format: date-time */
            scheduled_for: string;
            /** Format: uuid */
            tick_id: string;
            /** Format: uuid */
            trigger_id: string;
        };
        /** @description Operator changes are immutable facts; clearing an override restores the published code default. */
        TriggerChange: {
            /** @enum {string} */
            kind: "enable";
        } | {
            /** @enum {string} */
            kind: "pause";
        } | {
            /** @enum {string} */
            kind: "reset_default";
        } | {
            /** @enum {string} */
            kind: "set_cursor";
            /** Format: date-time */
            time: string;
        } | {
            /** @enum {string} */
            kind: "reset_cursor";
        };
        TriggerChangeRecord: {
            /** Format: date-time */
            occurred_at: string;
            request_json: components["schemas"]["TriggerChangeRequest"];
            /** Format: uuid */
            snapshot_id: string;
        };
        TriggerChangeRequest: {
            actor: string;
            change: components["schemas"]["TriggerChange"];
            /** Format: uuid */
            event_id: string;
            reason: string;
            /** Format: uuid */
            trigger_id: string;
        };
        /** @description Legacy schedule documents retain their stored shape; sensors add a distinct condition shape. */
        TriggerDeclaration: components["schemas"]["TriggerDefinition"] | components["schemas"]["SensorDefinition"];
        TriggerDefinition: {
            catch_up?: components["schemas"]["CatchUp"];
            default_status?: components["schemas"]["TriggerStatus"];
            description: string;
            job_key: string;
            schedule: components["schemas"]["Schedule"];
            /** Format: date-time */
            starts_at?: string | null;
        };
        TriggerRecord: {
            definition_json: components["schemas"]["TriggerDeclaration"];
            job_key: string;
            /** Format: date-time */
            last_tick_for?: string | null;
            name: string;
            /** Format: date-time */
            next_due_at?: string | null;
            status: components["schemas"]["TriggerStatus"];
            /** Format: uuid */
            trigger_id: string;
        };
        /** @enum {string} */
        TriggerStatus: "running" | "paused";
        /** @description Preserve the warehouse representation, including text-encoded measurements and booleans. */
        WarehouseAuditRecord: {
            attached_column_name?: string | null;
            attached_target_kind?: string | null;
            attached_target_name?: string | null;
            attachment_kind: string;
            audit_definition_name: string;
            audit_description?: string | null;
            audit_meta_json?: string | null;
            audit_name: string;
            binding_key: string;
            definition_fingerprint: string;
            evaluation_mode: string;
            /** Format: int64 */
            evidence_count?: number | null;
            evidence_error?: string | null;
            evidence_json?: string | null;
            evidence_sql?: string | null;
            evidence_truncated?: string | null;
            executed_sql?: string | null;
            execution_error?: string | null;
            execution_fingerprint: string;
            invocation_id: string;
            measured_value?: string | null;
            measurement_sql?: string | null;
            metadata_json?: string | null;
            /** Format: int64 */
            minimum_samples?: number | null;
            /** Format: date-time */
            occurred_at: string;
            outcome: string;
            result_id: string;
            reused: string;
            run_id: string;
            run_scope_phase: string;
            /** Format: int64 */
            sample_count?: number | null;
            sample_unit?: string | null;
            /** Format: int64 */
            schema_version: number;
            severity: string;
            sql_digest?: string | null;
            state_database?: string | null;
            state_schema: string;
            target_database?: string | null;
            target_name?: string | null;
            target_schema?: string | null;
            thresholds_json?: string | null;
            /** Format: int64 */
            violation_count?: number | null;
        };
        WarehouseCatalogColumn: {
            data_type: string;
            name: string;
        };
        /** @description Physical identity and ordered columns come from the adapter catalog, not fingerprints. */
        WarehouseCatalogRecord: {
            columns: components["schemas"]["WarehouseCatalogColumn"][];
            name: string;
            relation_type: string;
            state_database?: string | null;
            state_schema: string;
        };
        /** @description Fingerprints remain history here; latest-row selection belongs to the existing planner reader. */
        WarehouseFingerprintRecord: {
            definition: string;
            definition_hash: string;
            event_id?: string | null;
            metadata_json: string;
            node_name: string;
            node_type: string;
            run_id: string;
            schema_fingerprint: string;
            state_database?: string | null;
            state_schema: string;
            target_database?: string | null;
            target_name?: string | null;
            target_schema?: string | null;
            /** Format: date-time */
            ts: string;
            version_hash: string;
        };
        /** @description Preserve observations, not an inferred latest version, age status or run outcome. */
        WarehouseFreshnessRecord: {
            data_version?: string | null;
            data_version_hash: string;
            event_id?: string | null;
            /** Format: date-time */
            observed_at: string;
            run_id: string;
            source_name: string;
            state_database?: string | null;
            state_schema: string;
            strategy: string;
            target_database?: string | null;
            target_name?: string | null;
            target_schema?: string | null;
            value_kind: string;
        };
        /**
         * @description Catalog observations require full scans; only immutable history supports a watermark.
         * @enum {string}
         */
        WarehouseHistoryKind: "fingerprints" | "source_freshness" | "audit_results" | "janitor_events" | "migrations" | "old_name_views" | "microbatches" | "catalog";
        /** @description Warehouse cleanup facts do not authorize control-plane deletion or worker recovery. */
        WarehouseJanitorRecord: {
            archive_name: string;
            archive_qualified_name: string;
            /** Format: date-time */
            archived_at: string;
            event_id: string;
            event_type: string;
            /** Format: date-time */
            occurred_at: string;
            original_name?: string | null;
            original_qualified_name?: string | null;
            relation_database?: string | null;
            relation_schema: string;
            relation_type?: string | null;
            run_id: string;
            /** Format: int64 */
            schema_version: number;
            state_database?: string | null;
            state_schema: string;
        };
        /** @description Immutable coordination evidence, never authority to execute or mark a batch complete. */
        WarehouseMicrobatchRecord: {
            batch_size: string;
            /** Format: date-time */
            completed_at?: string | null;
            completion_type?: string | null;
            coverage_source?: string | null;
            /** Format: date-time */
            created_at: string;
            cursor_column: string;
            cursor_grain?: string | null;
            cursor_type: string;
            definition_hash?: string | null;
            event_id: string;
            execution_run_id: string;
            /** Format: date-time */
            execution_run_started_at?: string | null;
            fingerprint_status: string;
            model_name: string;
            model_version_hash?: string | null;
            /** Format: date-time */
            observed_at?: string | null;
            /** Format: int64 */
            observed_row_count?: number | null;
            origin_run_id: string;
            /** Format: date-time */
            origin_run_started_at?: string | null;
            partition_end?: string | null;
            partition_start?: string | null;
            physical_generation_id: string;
            previous_model_version_hash?: string | null;
            record_type: string;
            replay_policy?: string | null;
            replay_requirement_id?: string | null;
            required_model_version_hash?: string | null;
            /** Format: int64 */
            rows_affected?: number | null;
            run_end: string;
            run_start: string;
            run_type: string;
            scope_key: string;
            scope_kind: string;
            state_database?: string | null;
            state_schema: string;
            synthetic_reason?: string | null;
            target_database?: string | null;
            target_name: string;
            target_schema?: string | null;
            unaccounted_policy?: string | null;
        };
        /** @description Model-move history is evidence, not authority to migrate a warehouse relation. */
        WarehouseMigrationRecord: {
            /** Format: date-time */
            created_at: string;
            decision: string;
            destination_database?: string | null;
            destination_model: string;
            destination_name: string;
            destination_schema: string;
            discovery: string;
            event_id: string;
            origin_database?: string | null;
            origin_model?: string | null;
            origin_name: string;
            origin_schema?: string | null;
            origin_version_hash: string;
            run_id: string;
            state_database?: string | null;
            state_schema: string;
            target_name?: string | null;
        };
        /** @description Preserve nullable compatibility-view facts without projecting live warehouse existence. */
        WarehouseOldNameRecord: {
            archive_name?: string | null;
            column_aliases?: string | null;
            /** Format: date-time */
            created_at: string;
            destination_model: string;
            drop_reason?: string | null;
            event_id: string;
            event_type: string;
            /** Format: date-time */
            expires_at?: string | null;
            grants_copied?: string | null;
            migration_event_id: string;
            new_database?: string | null;
            new_name: string;
            new_schema: string;
            old_database?: string | null;
            old_name: string;
            old_schema?: string | null;
            run_id: string;
            state_database?: string | null;
            state_schema: string;
            target_name?: string | null;
            view_retention?: string | null;
            view_sql?: string | null;
        };
        WarehouseReconciliationCursor: {
            /** Format: uuid */
            event_id: string;
            /** Format: date-time */
            observed_at: string;
        };
        /** @description A complete, ordered key-set scan of one physical state table, optionally one UTC day. */
        WarehouseReconciliationPage: {
            after?: null | components["schemas"]["WarehouseReconciliationCursor"];
            complete: boolean;
            /** Format: date */
            day?: string | null;
            /** Format: int32 */
            page: number;
            /** Format: uuid */
            scan_id: string;
            state_database?: string | null;
            state_schema: string;
        };
        /** @description Physical namespace of SQLBuild's source-freshness state table, never a source query. */
        WarehouseStateLocation: {
            database?: string | null;
            schema: string;
        };
        /**
         * @description Closed failure stages keep warehouse errors and credentials out of health observations.
         * @enum {string}
         */
        WarehouseSyncFailure: "watermark" | "read" | "delivery";
        WarehouseSyncResponse: {
            applied: boolean;
            next_reconciliation_after?: null | components["schemas"]["WarehouseReconciliationCursor"];
        };
        /** @description Derive progress from committed mirror rows, scoped to the active physical reader. */
        WarehouseWatermarkRead: {
            /** Format: int64 */
            generation: number;
            kind: components["schemas"]["WarehouseHistoryKind"];
            location: components["schemas"]["WarehouseStateLocation"];
            /** Format: uuid */
            session_id: string;
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    changeAdministrator: {
        parameters: {
            query?: never;
            header: {
                /** @description Session CSRF token */
                "x-sqb-csrf": string;
            };
            path: {
                principal_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AdministratorChange"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    assignBuiltinRole: {
        parameters: {
            query?: never;
            header: {
                "x-sqb-csrf": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RoleAssignmentRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    logoutSession: {
        parameters: {
            query?: never;
            header: {
                /** @description Session CSRF token */
                "x-sqb-csrf": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
        };
    };
    oidcIdentity: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExternalIdentityResponse"];
                };
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
        };
    };
    loginOidc: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            303: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
        };
    };
    changePassword: {
        parameters: {
            query?: never;
            header: {
                /** @description Session CSRF token */
                "x-sqb-csrf": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PasswordChangeRequest"];
            };
        };
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
        };
    };
    loginPassword: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PasswordLoginRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PasswordSessionResponse"];
                };
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
        };
    };
    proxyIdentity: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExternalIdentityResponse"];
                };
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
        };
    };
    currentSession: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SessionIdentity"];
                };
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthenticationDenial"];
                };
            };
        };
    };
    readSecurityAudit: {
        parameters: {
            query?: {
                /** @description Exclusive storage-order cursor; pre-upgrade facts are ordered by timestamp and event ID */
                after?: number;
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SecurityAuditEvent"][];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    readPersonCatalog: {
        parameters: {
            query: {
                /** @description Exact database; omitted matches SQL NULL */
                database?: string;
                /** @description Exact physical schema */
                schema: string;
                after_observed_at?: string;
                after_event_id?: string;
                /** @description Page size; defaults to 100, clamped to 1..100 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CatalogPage"];
                };
            };
            /** @description Invalid namespace or cursor */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    listPersonRuns: {
        parameters: {
            query?: {
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunRecord"][];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    requestJobRun: {
        parameters: {
            query?: never;
            header: {
                "x-sqb-csrf": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["JobRunRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    cancelPersonRun: {
        parameters: {
            query?: never;
            header: {
                "x-sqb-csrf": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CancelRunRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    rerunPersonRun: {
        parameters: {
            query?: never;
            header: {
                "x-sqb-csrf": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RerunRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    changeTrigger: {
        parameters: {
            query?: never;
            header: {
                "x-sqb-csrf": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TriggerChangeRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    read_location_health: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LocationHealthObservation"][];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_location_health: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LocationHealthReport"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    subscribe: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description changed events: all, runs, run:<id>, ticks, snapshot:<location>, node_status:<target>. Refetch REST on all/reconnect; no durable cursor. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/event-stream": components["schemas"]["ChangeNotice"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_audit_results: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AuditResultsBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_catalog: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CatalogBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Reader generation fenced */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_fingerprints: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["FingerprintBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_janitor_events: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["JanitorEventsBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_microbatches: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MicrobatchesBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_migrations: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MigrationsBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_old_name_views: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["OldNameViewsBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_source_freshness: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SourceFreshnessBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    watermark: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["WarehouseWatermarkRead"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": string | null;
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Reader generation fenced */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    list_runs: {
        parameters: {
            query?: {
                /** @description Maximum recent runs; defaults to 100, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunRecord"][];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    invocations: {
        parameters: {
            query?: {
                /** @description Maximum invocation namespaces; defaults to 100, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
                run_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": unknown[];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run not found */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    nodes: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
                run_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunNodeRecord"][];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run not found */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    read_run_recall: {
        parameters: {
            query: {
                /** @description Exact scheduler or worker invocation ID */
                invocation: string;
                /** @description Maximum rows; defaults to 100, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
                run_id: string;
                view: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": unknown[];
                };
            };
            /** @description Unknown Recall view */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run or invocation not found */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    current: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CurrentSnapshot"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description No successful snapshot */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    publish: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SnapshotInput"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SnapshotRecord"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stale generation */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    metadata: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SnapshotRecord"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description No successful snapshot */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    list_triggers: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TriggerRecord"][];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    events: {
        parameters: {
            query?: {
                /** @description Maximum events; defaults to 100, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
                trigger_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TriggerChangeRecord"][];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Trigger not found */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    ticks: {
        parameters: {
            query?: {
                /** @description Maximum recent ticks; defaults to 100, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
                trigger_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TickRecord"][];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Trigger not found */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    poll: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LocationWorkPoll"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": null | components["schemas"]["LocationWorkClaim"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    result: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                request_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": null | components["schemas"]["LocationWorkRecord"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    complete: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                request_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LocationWorkResult"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Location not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Work claim expired or was replaced */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    report_run_event: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                run_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RunChangeRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    execution: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                run_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunExecution"];
                };
            };
            /** @description Inactive claim or invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    heartbeat: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                run_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HeartbeatResponse"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    output: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                run_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["OutputChunk"][];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OutputPublication"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    recall: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                run_id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RecallBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InsertOutcome"];
                };
            };
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Invalid credential */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Run not authorized */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
}
