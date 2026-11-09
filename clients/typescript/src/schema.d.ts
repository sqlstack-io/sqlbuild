// Generated from sqb server --openapi. Run npm run generate; do not edit.
export interface paths {
    "/api/admin/locations/{location_id}/targets/{target}/roles": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listRoleDefinitions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/people": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPeople"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
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
        get: operations["listRoleHoldings"];
        put?: never;
        post: operations["assignBuiltinRole"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/cli/approve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["approveCliLogin"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/cli/claim": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["claimCliLogin"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/cli/start": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["startCliLogin"];
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
    "/api/control/bootstrap": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readBootstrap"];
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
    "/api/control/locations/{location_id}/targets/{target}/changes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["subscribeChanges"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/checks": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonChecks"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonCheckHistory"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/results/{event_id}/evidence": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonCheckEvidence"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/sql": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonCheckSql"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/column-lineage": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonColumnLineage"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/connection-tests": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestConnectionTest"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/connection-tests/{request_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readConnectionTest"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/fingerprint": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonFingerprint"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonLocationHealth"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/jobs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonJobs"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/jobs/{job_key}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonJob"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/maintenance": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonMaintenance"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/microbatches/{model}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonMicrobatchCoverage"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/node-status": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonNodeStatus"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/nodes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonCatalogNodes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/runs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonNodeRuns"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/sql": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonNodeSql"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/plans": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestPlan"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPlan"];
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
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonRunEvents"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonRunNodes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes/{node_key}/error": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonRunNodeError"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonRunOutput"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/statements": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonRunStatements"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/statements/sql": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonRunStatementSql"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/snapshots": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonSnapshots"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/statistics": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonStatistics"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/triggers": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonTriggers"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/changes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonTriggerChanges"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonTicks"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/platform/problems": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonPlatformProblems"];
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
    "/api/control/runs/adhoc": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestAdhocRun"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/runs/adhoc/check": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["checkAdhocRun"];
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
    "/api/locations/{location_id}/targets/{target}/metadata": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_metadata"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/locations/{location_id}/targets/{target}/metadata/statistics-due": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["statistics_due"];
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
         * @description Closed permission vocabulary; unknown actions fail deserialization.
         * @enum {string}
         */
        Action: "view" | "view.sql" | "view.evidence" | "job.run" | "run.cancel" | "run.rerun" | "run.rewrite_history" | "run.prioritise" | "run.warehouse" | "trigger.toggle" | "sensor.reset_cursor" | "run.adhoc" | "connection.test" | "user.manage" | "role.assign" | "audit.view";
        /** @description What launching the command would run and whether this person may launch it. */
        AdhocCheck: {
            /** @description Why the command cannot run regardless of permissions, such as a refused flag or an unknown selector. */
            error?: string | null;
            launchable: boolean;
            permissions: components["schemas"]["AdhocPermission"][];
            selection: string[];
            settings: {
                [key: string]: unknown;
            };
            /** Format: uuid */
            snapshot_id?: string | null;
            verb?: string | null;
            write_set: string[];
        };
        /** @description A dry run of a command; nothing is queued or audited. */
        AdhocCheckRequest: {
            command: string[];
            /** Format: uuid */
            location_id: string;
            /** Format: int64 */
            priority: number;
            target: string;
        };
        /** @description One permission the run needs and whether the person holds it on every resolved node. */
        AdhocPermission: {
            action: components["schemas"]["Action"];
            allowed: boolean;
            /** @description Why it is refused, as the launch would report it. */
            reason?: string | null;
        };
        /** @description A person's command as argument tokens after `sqb`, never shell text; selectors resolve on the server. */
        AdhocRunRequest: {
            command: string[];
            /** Format: uuid */
            location_id: string;
            /** Format: int64 */
            priority: number;
            reason: string;
            /** Format: uuid */
            run_id: string;
            target: string;
        };
        /**
         * @description Administrator membership and person eligibility share the final-admin invariant.
         * @enum {string}
         */
        AdministratorChange: "grant" | "revoke" | "disable" | "enable";
        ApiErrorBody: {
            /**
             * Format: uuid
             * @description Existing durable denial audit ID; absent when no decision was recorded.
             */
            decision_id?: string | null;
            error: string;
        };
        AuthenticationDenial: {
            error: string;
            /** Format: uuid */
            request_id: string;
        };
        /**
         * @description How one recorded stretch of a microbatch model's cursor stands.
         * @enum {string}
         */
        BatchState: "complete" | "gap" | "replay_required";
        /** @description Every instance of this location target has missed two check-ins. */
        BrokenLocation: {
            /** Format: date-time */
            last_seen_at: string;
            /** Format: uuid */
            location_id: string;
            location_name: string;
            target: string;
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
        /** @description A fingerprint row's build identity; the `sqb` run that wrote it, not a scheduler run. */
        CatalogNodeBuild: {
            /** Format: date-time */
            observed_at: string;
            run_id: string;
            version_hash: string;
        };
        /** @description A current snapshot node's declared metadata and latest build, without SQL, source or config. */
        CatalogNodeRecord: {
            column_count?: number | null;
            /** @description Keys of the nodes this node reads. */
            dependencies: string[];
            description?: string | null;
            /** @description The snapshot node kind, such as `model`, `seed`, `source` or `audit`. */
            kind: string;
            last_build?: null | components["schemas"]["CatalogNodeBuild"];
            materialization?: string | null;
            name: string;
            node_key: string;
            path?: string | null;
            relation?: null | components["schemas"]["CatalogRelation"];
            /** Format: uuid */
            snapshot_id: string;
            tags: string[];
        };
        CatalogObservation: {
            /** Format: uuid */
            event_id: string;
            /** Format: date-time */
            observed_at: string;
            record: components["schemas"]["WarehouseCatalogRecord"];
        };
        CatalogPage: {
            next_after?: null | components["schemas"]["WarehouseMirrorCursor"];
            records: components["schemas"]["CatalogObservation"][];
        };
        CatalogRelation: {
            database?: string | null;
            name: string;
            schema: string;
        };
        /** @description SSE invalidation topics; `all` means refetch the authorized REST resources. */
        ChangeNotice: {
            topic: string;
        };
        /** @description One result's stored evidence rows; `view.evidence` reads them, audited. */
        CheckEvidence: {
            binding_key: string;
            /** @description Every row key, in sorted order; the warehouse stores rows with sorted keys. */
            columns: string[];
            /** Format: uuid */
            event_id: string;
            /** @description Scalar cells aligned to `columns`; a row without a key holds null there. */
            rows: Record<string, never>[][];
            truncated?: string | null;
        };
        /** @description One stored check result. Its SQL, error texts and evidence rows need their own permissions. */
        CheckResult: {
            attached_column_name?: string | null;
            attached_target_kind?: string | null;
            attached_target_name?: string | null;
            attachment_kind: string;
            audit_definition_name: string;
            audit_description?: string | null;
            audit_name: string;
            /** @description Identity of the check; a renamed model or changed severity or thresholds starts a new one. */
            binding_key: string;
            evaluation_mode: string;
            /** Format: uuid */
            event_id: string;
            /** Format: int64 */
            evidence_count?: number | null;
            evidence_truncated?: string | null;
            invocation_id: string;
            measured_value?: string | null;
            /** Format: int64 */
            minimum_samples?: number | null;
            /** Format: date-time */
            occurred_at: string;
            outcome: string;
            reused: string;
            run_id: string;
            run_scope_phase: string;
            /** Format: int64 */
            sample_count?: number | null;
            sample_unit?: string | null;
            severity: string;
            thresholds_json?: string | null;
            /** Format: int64 */
            violation_count?: number | null;
        };
        /** @description A check's latest SQL and error texts, which quote SQL; `view.sql` reads them, audited. */
        CheckSql: {
            binding_key: string;
            /** Format: uuid */
            event_id: string;
            evidence_error?: string | null;
            evidence_sql?: string | null;
            executed_sql?: string | null;
            execution_error?: string | null;
            measurement_sql?: string | null;
        };
        CliLoginApprovalRequest: {
            approve: boolean;
            browser_code: string;
        };
        CliLoginClaimRequest: {
            client_secret: string;
            /** Format: uuid */
            flow_id: string;
        };
        CliLoginSessionResponse: {
            bearer: string;
            identity: components["schemas"]["SessionIdentity"];
        };
        CliLoginStartRequest: {
            client_hash: string;
        };
        /** @description One published column lineage edge; `transform` is the compiler's transform kind, such as `direct` or `cast`. */
        ColumnLineageEdge: {
            source_column: string;
            source_key: string;
            target_column: string;
            target_key: string;
            transform: string;
        };
        /** @description The column lineage reaching or leaving one node, from the current snapshot; empty before one publishes lineage. */
        ColumnLineageRead: {
            edges: components["schemas"]["ColumnLineageEdge"][];
            nodes: components["schemas"]["LineageNodeColumns"][];
            /** Format: uuid */
            snapshot_id?: string | null;
        };
        /** @description A row whose key was already stored with different content; the first row is kept. */
        ConflictEvidence: {
            existing_sha256: string;
            record_key: string;
            rejected_sha256: string;
            table: components["schemas"]["Table"];
        };
        ConnectionTestRequest: {
            reason: string;
            /** Format: uuid */
            request_id: string;
        };
        /** @description What the UI needs first: how people sign in, and what this person may see and do. */
        ControlBootstrap: {
            access: components["schemas"]["PersonAccess"];
            login_mode: components["schemas"]["LoginMode"];
        };
        /** @description One stretch of the cursor, as raw bound texts. */
        CoverageBatch: {
            end: string;
            /** @description The run that executed the latest completion here; absent for gaps and external completions. */
            run_id?: string | null;
            start: string;
            state: components["schemas"]["BatchState"];
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
        /** @description Local definition identity only, not upstream data or graph currency. */
        DefinitionEvidence: {
            local_hash: string;
            /** @enum {string} */
            status: "matched";
        } | {
            current_local_hash: string;
            recorded_local_hash: string;
            /** @enum {string} */
            status: "changed";
        } | {
            current_local_hash?: string | null;
            reason: string;
            /** @enum {string} */
            status: "unavailable";
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
        /** @description Build evidence only: no SQL, arbitrary metadata, or inference about live relation existence. */
        FingerprintObservation: {
            definition_hash: string;
            /** Format: uuid */
            event_id: string;
            /** Format: date-time */
            observed_at: string;
            run_id: string;
            schema_fingerprint: string;
            source_event_id: string;
            version_hash: string;
        };
        /** @description A role every member of an SSO group holds. */
        GroupRole: {
            group_name: string;
            /** Format: uuid */
            location_id?: string | null;
            role_key: string;
            target?: string | null;
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
        /** @description One job or group as declared, for a person who may list the target; free-form setting values are withheld. */
        JobDetailRecord: {
            concurrency_groups: string[];
            /** @description Selected nodes whose failure stops a group. */
            critical_nodes: string[];
            description: string;
            job_key: string;
            kind: components["schemas"]["RunKind"];
            /** @description The groups this job belongs to. */
            member_of: string[];
            /** @description A group's member jobs in run order; empty for a job. */
            members: components["schemas"]["JobMember"][];
            selection: string[];
            /** @description Command settings; `vars` and a `defer_*` value that is not a target name show as `[REDACTED]`, and text values pass through secret redaction. */
            settings: {
                [key: string]: unknown;
            };
            /** Format: uuid */
            snapshot_id: string;
        };
        JobMember: {
            job_key: string;
            /** @description Members this one waits for, from lineage and from the group's `after`. */
            waits_for: string[];
        };
        /** @description A job or group of the current published snapshot. Settings are left out of person reads. */
        JobRecord: {
            concurrency_groups: string[];
            description: string;
            job_key: string;
            /** @description `job` or `group`; a group's selection is the union of its member jobs' selections. */
            kind: components["schemas"]["RunKind"];
            selection: string[];
            /** Format: uuid */
            snapshot_id: string;
        };
        /** @description A trusted operator names a published job; execution inputs are never supplied by the caller. */
        JobRunRequest: {
            /** @description Leave nodes out of the job with `sqb build` selectors. */
            exclude?: string[];
            /** @description Rebuild every selected model from scratch; it rewrites history, as a window does. */
            full_refresh?: boolean;
            job_key: string;
            /** Format: uuid */
            location_id: string;
            /** Format: int64 */
            priority: number;
            reason: string;
            /** Format: uuid */
            run_id: string;
            /** @description Narrow the job with `sqb build` selectors; only nodes already in the job can remain. */
            select?: string[];
            target: string;
            window?: null | components["schemas"]["RunWindow"];
        };
        LineageColumn: {
            data_type?: string | null;
            name: string;
        };
        /** @description A node's columns as its compiled definition infers them; `data_type` is absent when inference could not tell. */
        LineageNodeColumns: {
            columns: components["schemas"]["LineageColumn"][];
            kind: string;
            node_key: string;
        };
        /** @description One location target the person can view, with the actions allowed on at least one of its nodes. */
        LocationAccess: {
            actions: components["schemas"]["Action"][];
            /** Format: uuid */
            location_id: string;
            name: string;
            target: string;
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
        } | {
            command: string[];
            /** @enum {string} */
            kind: "plan";
            selection: string[];
            settings: Record<string, never>;
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
            document: Record<string, never>;
            /** @enum {string} */
            kind: "plan";
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
        /**
         * @description How people sign in to this server.
         * @enum {string}
         */
        LoginMode: "password" | "trusted_proxy" | "oidc";
        /** @description Recorded moves, their old-name view steps and janitor cleanups, newest moves first. */
        MaintenanceHistory: {
            /** @description Empty when the history is for one model; cleanups are not tied to models. */
            janitor_events: components["schemas"]["WarehouseJanitorRecord"][];
            migrations: components["schemas"]["WarehouseMigrationRecord"][];
            /** @description Every step recorded for the listed moves, oldest first; same-time steps are not in step order. */
            old_name_views: components["schemas"]["MaintenanceOldNameView"][];
        };
        /** @description One old-name view step as people read it: copied grants are counted, never returned as statements. */
        MaintenanceOldNameView: {
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
            /**
             * Format: int64
             * @description How many grants were copied onto the view; null until it exists or when the stored list is unreadable.
             */
            grants_copied?: number | null;
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
        /** @description One bounded page of one state table from the registered reader of a published generation. */
        MetadataBatch: {
            /** Format: int64 */
            generation: number;
            kind: components["schemas"]["WarehouseHistoryKind"];
            records: Record<string, never>[];
            scan?: null | components["schemas"]["MetadataScan"];
            /** Format: uuid */
            session_id: string;
        };
        /** @description A full scan's complete page removes the namespace's rows that the scan did not deliver. */
        MetadataScan: {
            complete: boolean;
            location: components["schemas"]["WarehouseStateLocation"];
            /** Format: uuid */
            scan_id: string;
        };
        /** @description Recorded coverage of one model's current relation; tracked only for concurrent microbatch models. */
        MicrobatchCoverage: {
            batch_size: string;
            /** @description The newest batches, oldest first. */
            batches: components["schemas"]["CoverageBatch"][];
            /** @description Whether older batches were left out of `batches`. */
            batches_truncated: boolean;
            /** @description End of the completed range that starts at `earliest` without a hole. */
            contiguous_frontier?: string | null;
            cursor_column: string;
            cursor_type: string;
            /** @description Start of the earliest completed partition. */
            earliest: string;
            /** @description Holes between completed partitions, oldest first. */
            known_gaps: components["schemas"]["CoverageBatch"][];
            model: string;
            physical_generation_id: string;
            target_name: string;
        };
        MutationResponse: {
            applied: boolean;
        };
        /** @enum {string} */
        NodeEventKind: "success" | "failed" | "recovered";
        /** @enum {string} */
        NodeOutcome: "success" | "warn" | "failed" | "skipped" | "cancelled";
        /** @description One build attempt of a node by a run in this target; the same metadata as the run's node list. */
        NodeRunRecord: {
            /** Format: date-time */
            ended_at?: string | null;
            error_code?: string | null;
            job_key?: string | null;
            /** Format: date-time */
            queued_at: string;
            /** Format: int64 */
            rows_affected?: number | null;
            /** Format: uuid */
            run_id: string;
            skipped_because_of?: string | null;
            /** Format: date-time */
            started_at?: string | null;
            status: string;
        };
        /** @description SQL inspection never returns Python source, execution artifacts or unrelated node metadata. */
        NodeSql: {
            compiled_sql?: string | null;
            node_key: string;
            raw_sql?: string | null;
            /** Format: uuid */
            snapshot_id: string;
        };
        NodeStatus: {
            build?: null | components["schemas"]["FingerprintObservation"];
            definition: components["schemas"]["DefinitionEvidence"];
            node_key: string;
            physical: components["schemas"]["PhysicalEvidence"];
            /** Format: uuid */
            snapshot_id: string;
            source_data: components["schemas"]["SourceDataStatus"];
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
        /** @description A running schedule whose next tick is more than 30 seconds late. */
        OverdueTick: {
            /** Format: uuid */
            location_id: string;
            location_name: string;
            name: string;
            /** Format: date-time */
            scheduled_for: string;
            target: string;
            /** Format: uuid */
            trigger_id: string;
        };
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
        /** @description The person's name, the actions allowed server-wide, and the location targets they can view. */
        PersonAccess: {
            actions: components["schemas"]["Action"][];
            locations: components["schemas"]["LocationAccess"][];
            name: string;
            /** Format: uuid */
            principal_id: string;
        };
        /** @description A role assigned to one person, everywhere or on one location and target. */
        PersonRole: {
            /** Format: uuid */
            assignment_id: string;
            /** Format: uuid */
            location_id?: string | null;
            name: string;
            /** Format: uuid */
            principal_id: string;
            role_key: string;
            target?: string | null;
        };
        /** @description A person as administrators see them: how they sign in, their SSO groups and live sessions. */
        PersonSummary: {
            /**
             * Format: int64
             * @description Unrevoked, unexpired browser and CLI sessions; none while the person is disabled.
             */
            active_sessions: number;
            /** Format: date-time */
            created_at: string;
            /** Format: date-time */
            disabled_at?: string | null;
            /** @description Groups the identity provider reported at the person's last SSO sign-in. */
            groups: string[];
            has_password: boolean;
            /** Format: date-time */
            last_signed_in_at?: string | null;
            name: string;
            /** Format: uuid */
            principal_id: string;
        };
        /** @description One operator change of a trigger, as a person reads it: what changed, who changed it, when and why. */
        PersonTriggerChange: {
            /** @description The audited actor, such as `principal:<id>` for a person. */
            actor: string;
            /** @description The person's name when the actor is a person who still exists. */
            actor_name?: string | null;
            change: components["schemas"]["TriggerChange"];
            /**
             * Format: uuid
             * @description The change's own ID, as recorded in the security audit.
             */
            event_id: string;
            /** Format: date-time */
            occurred_at: string;
            /** @description The operator's reason, with credential-shaped text redacted. */
            reason: string;
            /**
             * Format: uuid
             * @description The publication that was current when the change was made.
             */
            snapshot_id: string;
        };
        /** @description A trigger as people list it, with its newest tick so a list needs no read per trigger. */
        PersonTriggerRecord: components["schemas"]["TriggerRecord"] & {
            latest_tick?: null | components["schemas"]["TickRecord"];
        };
        /** @description Observed physical evidence is not a live warehouse-existence guarantee. */
        PhysicalEvidence: {
            observation: components["schemas"]["CatalogObservation"];
            /** @enum {string} */
            status: "observed";
        } | {
            reason: string;
            /** @enum {string} */
            status: "unavailable";
        };
        /** @description A person's build command to plan on the location; the server freezes it like an ad-hoc run. */
        PlanRequest: {
            command: string[];
            /** Format: uuid */
            request_id: string;
        };
        /** @enum {string} */
        PlatformEventKind: "heartbeat" | "scheduler_no_leader" | "tick_overdue" | "calendar_horizon_low" | "location_broken" | "launch_failed";
        /** @description Problems derived from live state when read; nothing here is stored. */
        PlatformProblems: {
            broken_locations: components["schemas"]["BrokenLocation"][];
            overdue_ticks: components["schemas"]["OverdueTick"][];
            scheduler_leader: boolean;
            /**
             * Format: date-time
             * @description When the last scheduler lease ran out, or will run out; absent until a server first leads.
             */
            scheduler_lease_expires_at?: string | null;
        };
        /** @description A changed subject, such as `model:orders`, or `role:analyst` (`role:` for those who can't assign roles). */
        PublishedChange: {
            change_kind: string;
            subject_key: string;
        };
        /** @description One publication as people browse it: its commit, environment and changed definitions. */
        PublishedSnapshot: {
            /** @description Added, changed and removed nodes and roles, in publication order. */
            changes: components["schemas"]["PublishedChange"][];
            /** @description Digest of the lock file, Python and `sqb` versions; a change means dependencies changed. */
            environment_hash: string;
            /** Format: int64 */
            generation: number;
            git_author?: string | null;
            git_dirty: boolean;
            git_revision?: string | null;
            /** Format: date-time */
            published_at: string;
            /** Format: uuid */
            snapshot_id: string;
            sqb_version: string;
        };
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
            role: components["schemas"]["RoleKey"];
            target?: string | null;
        };
        /** @description Who holds which role where: assigned to people here, or mapped to SSO groups in server config. */
        RoleHoldings: {
            assignments: components["schemas"]["PersonRole"][];
            group_roles: components["schemas"]["GroupRole"][];
        };
        /** @description A built-in role by name, or a custom role a location publishes. */
        RoleKey: components["schemas"]["BuiltinRole"] | string;
        /** @description A role usable on one location target: built in, or published by the location's project. */
        RoleOverview: {
            builtin: boolean;
            /** @description The project's description; absent for built-in roles. */
            description?: string | null;
            /** @description Built-in roles: one grant without selectors, meaning every node. Custom roles: their published grants. */
            grants: components["schemas"]["RoleOverviewGrant"][];
            /**
             * Format: int64
             * @description Active people holding the role here or everywhere, assigned directly or through an SSO group.
             */
            holders: number;
            key: string;
        };
        /** @description Node selectors, the actions they grant, and the nodes each selector matched when published. */
        RoleOverviewGrant: {
            matched: {
                [key: string]: string[];
            };
            nodes: string[];
            permissions: components["schemas"]["Action"][];
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
            /** @description Redacted and bounded on the way in, whoever sends it; absent from older workers. */
            error_message?: string | null;
            /** @enum {string} */
            kind: "node_finished";
            node_key: string;
            outcome: components["schemas"]["NodeOutcome"];
            /** Format: int64 */
            rows_affected?: number | null;
            skipped_because_of?: string | null;
        } | {
            /** @enum {string} */
            kind: "check_started";
            node_key: string;
        } | {
            /** @enum {string} */
            kind: "check_finished";
            node_key: string;
            outcome: components["schemas"]["NodeOutcome"];
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
        /**
         * @description The `sqb` command a run executes; job and group runs always build.
         * @enum {string}
         */
        RunCommand: "build" | "test" | "seed" | "audit" | "freshness";
        RunErrorText: {
            error_type?: string | null;
            invocation_id: string;
            message: string;
            occurred_at?: string | null;
        };
        /** @enum {string} */
        RunEventKind: "completed" | "success" | "warn" | "partial" | "failed" | "cancelled";
        /** @description Recall event metadata of a run. Payloads are left out: they can carry SQL, request settings and error text. */
        RunEventRecord: {
            /** Format: int64 */
            affected_rows?: number | null;
            /** Format: double */
            duration_ms?: number | null;
            error_code?: string | null;
            error_type?: string | null;
            event_id: string;
            event_type: string;
            invocation_id: string;
            occurred_at: string;
            outcome?: string | null;
            producer?: string | null;
            resource_attempt_id?: string | null;
            resource_id?: string | null;
            resource_kind?: string | null;
            resource_name?: string | null;
            /** Format: int64 */
            row_count?: number | null;
            severity?: string | null;
            statement_id?: string | null;
            /** Format: int64 */
            storage_id: number;
        };
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
        /** @description The `sqb build` selectors one run of a job was narrowed with. */
        RunNarrowing: {
            exclude: string[];
            select: string[];
        };
        /** @description A node's outcome error in one run: redacted, bounded text and the project file of the node, never its SQL. */
        RunNodeFailure: {
            error_code?: string | null;
            /** @description Null when the worker reported none or the run's history has been pruned. */
            message?: string | null;
            node_key: string;
            /** @description Relative to the project root, with `/` separators; null when the run's snapshot is gone. */
            path?: string | null;
        };
        RunNodeRecord: {
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
        RunOutcome: "success" | "warn" | "partial" | "failed" | "cancelled";
        /** @description Captured output and error text of a run's invocations; both can quote warehouse data. */
        RunOutput: {
            /** @description Invocation failures and error diagnostics, oldest first. */
            errors: components["schemas"]["RunErrorText"][];
            /** @description The newest captured lines per invocation, oldest first. */
            lines: components["schemas"]["RunOutputLine"][];
        };
        RunOutputLine: {
            invocation_id: string;
            occurred_at: string;
            /** Format: int64 */
            sequence: number;
            stream: string;
            text: string;
        };
        RunRecord: {
            /** Format: date-time */
            cancel_requested_at?: string | null;
            /** Format: date-time */
            ended_at?: string | null;
            job_key?: string | null;
            kind: components["schemas"]["RunKind"];
            /** Format: int64 */
            last_event_storage_id: number;
            /** Format: date-time */
            launched_at?: string | null;
            /** Format: uuid */
            location_id: string;
            narrowing?: null | components["schemas"]["RunNarrowing"];
            node_counts_json: {
                [key: string]: number;
            };
            origin: components["schemas"]["RunOrigin"];
            /** Format: uuid */
            parent_run_id?: string | null;
            /** Format: int64 */
            priority: number;
            /** Format: date-time */
            queued_at: string;
            /** @description The stored actor: `principal:<id>` for a person, otherwise a system actor such as `system:scheduler`. */
            requested_by: string;
            /** @description The display name of the requesting principal; never set for system actors. */
            requester_name?: string | null;
            /** Format: uuid */
            rerun_of_run_id?: string | null;
            rewrite_history: boolean;
            /** Format: uuid */
            root_run_id: string;
            /** Format: uuid */
            run_id: string;
            selection_json: string[];
            /** Format: uuid */
            snapshot_id: string;
            /** Format: date-time */
            started_at?: string | null;
            status: components["schemas"]["RunStatus"];
            status_reason?: string | null;
            target: string;
            /** Format: uuid */
            tick_id?: string | null;
            /** Format: uuid */
            trigger_id?: string | null;
            trigger_name?: string | null;
        };
        /** @description The authorized selection and pinned execution inputs are immutable once requested. */
        RunRequest: {
            command?: components["schemas"]["RunCommand"];
            concurrency_groups: string[];
            image_id?: string | null;
            job_key?: string | null;
            kind: components["schemas"]["RunKind"];
            /** Format: uuid */
            location_id: string;
            narrowing?: null | components["schemas"]["RunNarrowing"];
            origin: components["schemas"]["RunOrigin"];
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
        /** @description Warehouse statement metadata of a run: kind, timing, counts and outcome, never the SQL text. */
        RunStatementRecord: {
            /** Format: int64 */
            affected_rows?: number | null;
            /** Format: double */
            duration_ms?: number | null;
            ended_at?: string | null;
            error_code?: string | null;
            error_type?: string | null;
            intent?: string | null;
            invocation_id: string;
            outcome?: string | null;
            purpose?: string | null;
            query_id?: string | null;
            resource_id?: string | null;
            resource_kind?: string | null;
            resource_name?: string | null;
            /** Format: int64 */
            row_count?: number | null;
            sql_available: boolean;
            sql_unavailable_reason?: string | null;
            started_at?: string | null;
            statement_id: string;
            statement_kind?: string | null;
        };
        /** @description One statement's executed SQL; `sql` is null when capture was off or retention dropped it. */
        RunStatementSql: {
            sql?: string | null;
            statement_id: string;
        };
        /** @enum {string} */
        RunStatus: "queued" | "launched" | "running" | "unresponsive" | "success" | "warn" | "partial" | "failed" | "cancelled";
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
            /** @description The person's name when `actor` is a principal; person reads set it, exports leave it out. */
            actor_name?: string | null;
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
            default_status?: components["schemas"]["TriggerStatus"];
            description: string;
            job_key: string;
            /** Format: int32 */
            minimum_interval_seconds: number;
            when: components["schemas"]["SensorCondition"];
        };
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
        SourceDataStatus: {
            reason: string;
            /** @enum {string} */
            status: "unknown";
        };
        StartedLogin: {
            browser_code: string;
            /** Format: date-time */
            expires_at: string;
        };
        /** @description Eligibility comes from each relation's committed hour, never a namespace watermark. */
        StatisticsDueRead: {
            /** Format: uuid */
            after?: string | null;
            /** Format: int64 */
            generation: number;
            limit: number;
            location: components["schemas"]["WarehouseStateLocation"];
            /** Format: uuid */
            session_id: string;
        };
        StatisticsObservation: {
            /** Format: uuid */
            event_id: string;
            record: components["schemas"]["WarehouseStatisticsRecord"];
        };
        StatisticsPage: {
            next_after?: null | components["schemas"]["WarehouseMirrorCursor"];
            records: components["schemas"]["StatisticsObservation"][];
        };
        /** @description One bounded catalog page; sampled and unsampled relations both advance its cursor. */
        StatisticsRead: {
            location: components["schemas"]["WarehouseStateLocation"];
            names: string[];
            /** Format: uuid */
            next_after?: string | null;
        };
        /**
         * @description The tables Recall writes.
         * @enum {string}
         */
        Table: "invocations" | "output" | "events" | "diagnostics" | "artifacts" | "artifact_refs" | "heartbeats" | "delivery" | "meta" | "conflicts";
        /** @enum {string} */
        TickOutcome: "requested" | "covered";
        TickRecord: {
            /** Format: uuid */
            covered_by_run_id?: string | null;
            /** Format: int64 */
            epoch: number;
            /** Format: date-time */
            evaluated_at: string;
            outcome: components["schemas"]["TickOutcome"];
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
        /**
         * @description Catalog observations require full scans; only immutable history supports a watermark.
         * @enum {string}
         */
        WarehouseHistoryKind: "fingerprints" | "source_freshness" | "audit_results" | "janitor_events" | "migrations" | "old_name_views" | "microbatches" | "catalog" | "statistics";
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
        WarehouseMirrorCursor: {
            /** Format: uuid */
            event_id: string;
            /** Format: date-time */
            observed_at: string;
        };
        /** @description Physical namespace of SQLBuild's source-freshness state table, never a source query. */
        WarehouseStateLocation: {
            database?: string | null;
            schema: string;
        };
        /** @description A metadata estimate at collection time, never evidence of source-data currency. */
        WarehouseStatisticsRecord: {
            /** Format: int64 */
            byte_count?: number | null;
            byte_count_basis?: string | null;
            detail?: string | null;
            name: string;
            /** Format: date-time */
            observed_at: string;
            /** Format: int64 */
            row_count?: number | null;
            state_database?: string | null;
            state_schema: string;
        };
        /**
         * @description Closed failure stages keep warehouse errors and credentials out of health observations.
         * @enum {string}
         */
        WarehouseSyncFailure: "watermark" | "read" | "delivery";
        WarehouseSyncResponse: {
            applied: boolean;
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
    listRoleDefinitions: {
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
            /** @description Built-in roles, then the target's published custom roles */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RoleOverview"][];
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
    listPeople: {
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
                    "application/json": components["schemas"]["PersonSummary"][];
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
    listRoleHoldings: {
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
                    "application/json": components["schemas"]["RoleHoldings"];
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
    approveCliLogin: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CliLoginApprovalRequest"];
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
    claimCliLogin: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CliLoginClaimRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CliLoginSessionResponse"];
                };
            };
            202: {
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
    startCliLogin: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CliLoginStartRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StartedLogin"];
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
                /** @description Exclusive storage-order cursor */
                after?: number;
                /** @description Exclusive upper cursor; when set, the page is newest first */
                before?: number;
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
    readBootstrap: {
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
                    "application/json": components["schemas"]["ControlBootstrap"];
                };
            };
            /** @description Session rejected or person disabled */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
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
    subscribeChanges: {
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
            /** @description changed events: all, runs, ticks, snapshot:<location>, node_status:<target>. Refetch REST on all/reconnect; no durable cursor. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/event-stream": components["schemas"]["ChangeNotice"];
                };
            };
            /** @description Session rejected or person disabled */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description View does not cover every node of this target */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    listPersonChecks: {
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
            /** @description The newest result of every check */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckResult"][];
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
    readPersonCheckHistory: {
        parameters: {
            query?: {
                /** @description Maximum results, newest first; defaults to 50, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
                binding_key: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Empty for an unknown check */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckResult"][];
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
    readPersonCheckEvidence: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
                binding_key: string;
                event_id: string;
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
                    "application/json": null | components["schemas"]["CheckEvidence"];
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
    readPersonCheckSql: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
                binding_key: string;
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
                    "application/json": null | components["schemas"]["CheckSql"];
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
    readPersonColumnLineage: {
        parameters: {
            query: {
                /** @description The node whose upstream and downstream column lineage to return */
                node_key: string;
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
                    "application/json": components["schemas"]["ColumnLineageRead"];
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
    requestConnectionTest: {
        parameters: {
            query?: never;
            header: {
                "x-sqb-csrf": string;
            };
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConnectionTestRequest"];
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
    readConnectionTest: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
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
    readPersonFingerprint: {
        parameters: {
            query: {
                node_type: string;
                node_name: string;
                /** @description Exact destination database; omitted matches SQL NULL */
                database?: string;
                /** @description Exact destination schema; omitted matches SQL NULL */
                schema?: string;
                /** @description Exact destination name; omitted matches SQL NULL */
                name?: string;
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
                    "application/json": null | components["schemas"]["FingerprintObservation"];
                };
            };
            /** @description Invalid node */
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
    readPersonLocationHealth: {
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
            /** @description This target's newest health observations and instance check-ins */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LocationHealthObservation"][];
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
    listPersonJobs: {
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
                    "application/json": components["schemas"]["JobRecord"][];
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
    readPersonJob: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
                job_key: string;
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
                    "application/json": components["schemas"]["JobDetailRecord"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Needs view on every current node of the target */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description No published job by that name */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    readPersonMaintenance: {
        parameters: {
            query?: {
                /** @description Only moves into this model, and no janitor events */
                node?: string;
                /** @description Maximum moves and janitor events each; defaults to 100, capped at 1000 */
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
                    "application/json": components["schemas"]["MaintenanceHistory"];
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
    readPersonMicrobatchCoverage: {
        parameters: {
            query?: {
                /** @description Maximum newest batches; defaults to 200, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
                model: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Null when the model has no tracked coverage */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": null | components["schemas"]["MicrobatchCoverage"];
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
    readPersonNodeStatus: {
        parameters: {
            query: {
                /** @description One exact published node key */
                node_key: string;
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
                    "application/json": null | components["schemas"]["NodeStatus"];
                };
            };
            /** @description Invalid node key */
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
    listPersonCatalogNodes: {
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
            /** @description Every node of the current snapshot; empty before the first publication */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CatalogNodeRecord"][];
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
    listPersonNodeRuns: {
        parameters: {
            query?: {
                /** @description Maximum recent runs; defaults to 100, capped at 1000 */
                limit?: number;
            };
            header?: never;
            path: {
                location_id: string;
                target: string;
                node_key: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Runs that built the node, newest first */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NodeRunRecord"][];
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
    readPersonNodeSql: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
                node_key: string;
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
                    "application/json": null | components["schemas"]["NodeSql"];
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
    requestPlan: {
        parameters: {
            query?: never;
            header: {
                "x-sqb-csrf": string;
            };
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PlanRequest"];
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
    readPlan: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
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
    listPersonRunEvents: {
        parameters: {
            query?: {
                /** @description Return events after this storage cursor */
                after?: number;
                /** @description Maximum events; defaults to 1000, capped at 5000 */
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
            /** @description Event metadata oldest first, without payloads */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunEventRecord"][];
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
            /** @description Run not found for a person who may view the whole target */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    listPersonRunNodes: {
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
            /** @description Run not found for a person who may view the whole target */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    readPersonRunNodeError: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
                run_id: string;
                node_key: string;
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
                    "application/json": components["schemas"]["RunNodeFailure"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Needs view.evidence on the node */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Node not found in this run */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    readPersonRunOutput: {
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
                    "application/json": components["schemas"]["RunOutput"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Needs view.evidence on every node the run selected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Run not found for a person who may view the whole target */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    listPersonRunStatements: {
        parameters: {
            query?: {
                /** @description Maximum statements; defaults to 1000, capped at 5000 */
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
            /** @description Statement metadata in start order, without SQL */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunStatementRecord"][];
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
            /** @description Run not found for a person who may view the whole target */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    readPersonRunStatementSql: {
        parameters: {
            query: {
                /** @description One statement of this run */
                statement_id: string;
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
                    "application/json": components["schemas"]["RunStatementSql"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Needs view.sql on every node the run selected */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
            /** @description Run or statement not found */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    listPersonSnapshots: {
        parameters: {
            query?: {
                /** @description Maximum recent publications; defaults to 20, capped at 100 */
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
            /** @description Publications newest first, each with its changed nodes and roles */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PublishedSnapshot"][];
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
    readPersonStatistics: {
        parameters: {
            query: {
                /** @description Exact database; omitted matches SQL NULL */
                database?: string;
                schema: string;
                name: string;
                after_observed_at?: string;
                after_event_id?: string;
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
                    "application/json": components["schemas"]["StatisticsPage"];
                };
            };
            /** @description Invalid relation or cursor */
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
    listPersonTriggers: {
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
            /** @description Published triggers with their newest tick */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonTriggerRecord"][];
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
    listPersonTriggerChanges: {
        parameters: {
            query?: {
                /** @description Maximum recent changes; defaults to 100, capped at 1000 */
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
            /** @description Operator changes, newest first */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonTriggerChange"][];
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
            /** @description Trigger not found for a person who may view the whole target */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    listPersonTicks: {
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
            /** @description Trigger not found for a person who may view the whole target */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    readPersonPlatformProblems: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Live scheduler state, and broken locations and late ticks of targets the person lists */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlatformProblems"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
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
    requestAdhocRun: {
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
                "application/json": components["schemas"]["AdhocRunRequest"];
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
    checkAdhocRun: {
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
                "application/json": components["schemas"]["AdhocCheckRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdhocCheck"];
                };
            };
            /** @description Session rejected */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
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
    report_metadata: {
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
                "application/json": components["schemas"]["MetadataBatch"];
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
    statistics_due: {
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
                "application/json": components["schemas"]["StatisticsDueRead"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StatisticsRead"];
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
            /** @description Body over its wire or decompressed bound */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Content encoding other than gzip */
            415: {
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
