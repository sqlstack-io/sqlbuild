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
    "/api/admin/server": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readServerSettings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/service-accounts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Every service account by name with its working keys, never the keys themselves. Needs user.manage or role.assign. */
        get: operations["listServiceAccounts"];
        put?: never;
        /** @description Adds a service account with a built-in role everywhere and returns its first key, only this once. Needs user.manage and role.assign. */
        post: operations["createServiceAccount"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/service-accounts/{principal_id}/disabled": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Disabling refuses every key of the account at once; enabling accepts the same keys again. Needs user.manage. */
        post: operations["setServiceAccountDisabled"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/service-accounts/{principal_id}/keys": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Issues a new key, returned only this once. The replaced key works on for overlap_minutes (at most 1440), else stops now; a key already in an overlap stops now. Needs user.manage. */
        post: operations["rotateServiceAccountKey"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/service-accounts/{principal_id}/keys/{key_id}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Stops one working key now, such as an old key before its overlap ends. Needs user.manage. */
        post: operations["revokeServiceAccountKey"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/service-accounts/{principal_id}/role": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** @description Gives the account a built-in role everywhere in place of its other built-in roles everywhere. Needs role.assign. */
        post: operations["setServiceAccountRole"];
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
    "/api/auth/login-mode": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Answered only by a server without sign-in for people, so a browser or sqb login can say so instead of offering a password. */
        get: operations["readLoginMode"];
        put?: never;
        post?: never;
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
    "/api/auth/password/people": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createPasswordPerson"];
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
    "/api/control/locations/{location_id}/targets/{target}/acknowledgements": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listRunAcknowledgements"];
        put?: never;
        post: operations["acknowledgeRun"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/attention-acknowledgements": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listAttentionAcknowledgements"];
        put?: never;
        post: operations["acknowledgeAttention"];
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
    "/api/control/locations/{location_id}/targets/{target}/cost": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonCostSummary"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/cost/selection": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readCostSelection"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/cost/series": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readCostSeries"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/discovered": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readDiscovered"];
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
    "/api/control/locations/{location_id}/targets/{target}/node-states": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonNodeStates"];
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
    "/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/cost": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonNodeCosts"];
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
    "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}/diff": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPlanDiff"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}/rows": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPlanRows"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/plans/{request_id}/summary": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPlanSummary"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/providers": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonProviders"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/republish": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestRepublish"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/republish/{request_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readRepublish"];
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
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/cost": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonRunCost"];
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
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/ignored-audits": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonRunIgnoredAudits"];
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
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/download": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["downloadPersonRunOutput"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/index": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonRunOutputIndex"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/lines": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonRunOutputLines"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/output/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["searchPersonRunOutput"];
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
    "/api/control/locations/{location_id}/targets/{target}/selection": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["evaluateSelection"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/selection/runs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPastRuns"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/selection/vocabulary": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readSelectionVocabulary"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/locations/{location_id}/targets/{target}/sensors/{name}/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonSensorEvents"];
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
    "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/sensor": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["readPersonSensorState"];
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
    "/api/control/locations/{location_id}/targets/{target}/warehouse-syncs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["requestWarehouseSync"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/control/platform/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["listPersonPlatformEvents"];
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
    "/api/locations/{location_id}/targets/{target}/changed-build": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_changed_build"];
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
    "/api/locations/{location_id}/targets/{target}/metadata/preview-copy": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["preview_copy"];
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
    "/api/locations/{location_id}/targets/{target}/metadata/sync-due": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["sync_due"];
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
    "/api/runs/{run_id}/output/batches": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["output_batch"];
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
         * @description Why a tick ended before its function's answer was committed.
         * @enum {string}
         */
        AbandonReason: "unanswered" | "superseded" | "republished" | "paused" | "cursor_reset" | "leader_changed" | "skipped";
        /** @description Marks a run as handled for everyone who can see it, or withdraws the mark; retried with the same event ID it applies once. */
        AcknowledgementRequest: {
            acknowledged: boolean;
            /** Format: uuid */
            event_id: string;
            /** Format: uuid */
            run_id: string;
        };
        /**
         * @description Closed permission vocabulary; unknown actions fail deserialization.
         * @enum {string}
         */
        Action: "view" | "view.sql" | "view.evidence" | "job.run" | "run.cancel" | "run.rerun" | "run.rewrite_history" | "run.prioritise" | "run.warehouse" | "run.vars" | "run.load" | "trigger.toggle" | "sensor.reset_cursor" | "run.adhoc" | "connection.test" | "location.republish" | "user.manage" | "role.assign" | "audit.view";
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
            /**
             * Format: uuid
             * @description A new random (UUID v4) or time-ordered (UUID v7) ID; name-based (v5) IDs are refused as the server derives those.
             */
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
        /** @description A condition whose latest acknowledgement event marks it handled. */
        AttentionAcknowledgement: {
            /** Format: date-time */
            acknowledged_at: string;
            /** @description The person's name, or the audit actor when the person no longer exists. */
            acknowledged_by: string;
            /**
             * Format: uuid
             * @description The acknowledging audit event, which a withdrawal can name.
             */
            event_id: string;
            key: string;
            shape: unknown;
            /**
             * Format: date-time
             * @description When the acknowledgement lapses even if the condition never changes; none without a time limit.
             */
            until?: string | null;
        };
        /** @description Marks an ongoing condition handled as it looks now, or withdraws that; an event ID applies once. */
        AttentionAcknowledgementRequest: {
            acknowledged: boolean;
            /** Format: uuid */
            event_id: string;
            /** @description The condition's kind and subject, stable while it lasts, such as `stale_sources`. */
            key: string;
            /** @description Why, when it isn't a person's own choice, such as a withdrawal once the condition cleared. */
            reason?: string | null;
            /** @description What the condition looks like, as a JSON object of at most 8 KiB that later states are compared with. */
            shape: unknown;
            /**
             * Format: uuid
             * @description For a withdrawal, the acknowledgement event it withdraws: it applies only while that is still the latest.
             */
            withdraws?: string | null;
        };
        /** @description A target's condition acknowledgements, and how long one lasts while its condition is unchanged. */
        AttentionAcknowledgements: {
            acknowledgements: components["schemas"]["AttentionAcknowledgement"][];
            /**
             * Format: int64
             * @description The server's `attention_snooze` in seconds; none when acknowledgements have no time limit.
             */
            snooze_seconds?: number | null;
        };
        AuthenticationDenial: {
            error: string;
            /** Format: uuid */
            request_id: string;
        };
        BTreeMap: {
            [key: string]: {
                /** @description Its failed, cancelled and never-reached nodes, and its failed audits. */
                failed: string[];
                /**
                 * Format: uuid
                 * @description The run the typed ID names.
                 */
                run_id: string;
                /** @description Every node the run selected. */
                selected: string[];
                /** @description The nodes it skipped because something they depend on failed. */
                skipped: string[];
            };
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
        /** @description Checks without a result yet are not known to the server, so they are not counted. */
        CatalogNodeChecks: {
            /** @description Failed or errored. */
            failing: number;
            passing: number;
            total: number;
            warning: number;
        };
        /** @description `success`, `warn` or `failed`; `run_id` is the scheduler run, null for a newer build outside the scheduler. */
        CatalogNodeOutcome: {
            /** Format: date-time */
            at: string;
            /** Format: uuid */
            run_id?: string | null;
            status: string;
        };
        /** @description A current snapshot node's declared metadata and latest build, which the node listing leaves to node-states. */
        CatalogNodeRecord: {
            /** @description The checks on the node by their newest result. */
            checks: components["schemas"]["CatalogNodeChecks"];
            column_count?: number | null;
            /** @description The current definition differs from the one `last_build` recorded; false when either is unknown. */
            definition_changed: boolean;
            /** @description Keys of the nodes this node reads. */
            dependencies: string[];
            description?: string | null;
            /** @description The snapshot node kind, such as `model`, `seed`, `source` or `audit`. */
            kind: string;
            last_build?: null | components["schemas"]["CatalogNodeBuild"];
            last_outcome?: null | components["schemas"]["CatalogNodeOutcome"];
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
            /** @description `node_status` only: too many nodes changed, or the snapshot or changed build is new; refetch node states. */
            full?: boolean | null;
            /**
             * Format: int64
             * @description `node_status` only: the version this change applies to; refetch node states when it is not yours.
             */
            previous_version?: number | null;
            /** @description `node_status` only: the nodes whose state changed, absent when `full` is set. */
            states?: {
                [key: string]: components["schemas"]["NodeState"];
            } | null;
            topic: string;
            /**
             * Format: int64
             * @description `node_status` only: the state version after this change.
             */
            version?: number | null;
        };
        /** @description The nodes the default `sqb build --changed` runs, as the location last planned it from the synced copy. */
        ChangedBuild: {
            /** @description Nodes that run for a change of their own, as `kind:name`. */
            changed: string[];
            /** @description Nodes that run only because an upstream they read runs. */
            downstream: string[];
            /** @description Why the location's plan failed or which models it couldn't plan, with the nodes it did plan; absent when every model planned. */
            failing?: string | null;
            /**
             * Format: int64
             * @description The publication generation it was planned for.
             */
            generation: number;
            /**
             * Format: date-time
             * @description When the server received it.
             */
            reported_at: string;
        };
        /** @description A location's plan of the default `sqb build --changed`, fenced to its reader session and generation. */
        ChangedBuildReport: {
            changed: string[];
            downstream: string[];
            /** @description Why the plan failed (empty lists) or which models it couldn't plan (the lists of the rest). */
            failing?: string | null;
            /** Format: int64 */
            generation: number;
            /** Format: uuid */
            session_id: string;
        };
        /**
         * @description How a check, audit or SQL test that ran inside a run fell short.
         * @enum {string}
         */
        CheckEventKind: "failed" | "warn";
        /** @description One result's stored evidence rows; `view.evidence` reads them, audited. */
        CheckEvidence: {
            binding_key: string;
            /** @description Every row key, in sorted order; the warehouse stores rows with sorted keys. */
            columns: string[];
            /** Format: uuid */
            event_id: string;
            /** @description The warehouse's error, SQLSTATE included, when the audit's query could not run; redacted like a node's. */
            execution_error?: string | null;
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
            /** @description An error-severity failure its build let through instead of holding anything back. */
            ignored?: boolean;
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
        /**
         * @description When a location checks its warehouse connection on its own.
         * @enum {string}
         */
        ConnectionCheckTrigger: "startup" | "publish";
        /** @description A target's connection settings as its location publishes them: names only, never a secret. */
        ConnectionDetails: {
            account?: string | null;
            /** @description The adapter, such as `snowflake`, `postgres` or `duckdb`. */
            adapter: string;
            /** @description How the location signs in, by name only, such as `key_pair`, `password` or `oauth`. */
            auth_method?: string | null;
            /** @description The database the connection opens by default. */
            database?: string | null;
            /** @description The server host, or for a file database its path. */
            host?: string | null;
            role?: string | null;
            user?: string | null;
            warehouse?: string | null;
        };
        /** @description What the UI needs first: how people sign in, and what this person may see and do. */
        ControlBootstrap: {
            access: components["schemas"]["PersonAccess"];
            /** @description The display name a trusted proxy reported for this request, if it reports one. */
            display_name?: string | null;
            /** @description The email address a trusted proxy reported for this request, if it reports one. */
            email?: string | null;
            login_mode: components["schemas"]["LoginMode"];
            /** @description Where signing out goes when a trusted proxy owns the sign-in; absent means no sign-out link. */
            logout_url?: string | null;
        };
        /** @description One job's estimated query cost on one node: in the period, and in the period before it. */
        CostFactRecord: {
            /** Format: int64 */
            bytes_scanned: number;
            /** Format: double */
            credits: number;
            /** @description The node's folder as `costFolder` names it; empty at the top of `sql/models`. */
            folder: string;
            /** @description None for builds no server run started. */
            job?: string | null;
            /** @description `<kind>:<name>`, like catalog node keys. */
            node_key: string;
            /** Format: double */
            previous_credits: number;
            /** Format: double */
            previous_usd?: number | null;
            /** Format: int64 */
            queries: number;
            /**
             * Format: int64
             * @description Builds in the period, counted once each.
             */
            runs: number;
            /** Format: double */
            seconds: number;
            /** Format: double */
            usd?: number | null;
        };
        /** @description Estimated query cost of one group, such as a job, a warehouse size or a node. */
        CostGroup: {
            /** Format: double */
            credits: number;
            /** @description The group's name; None for builds no server run started, or a resource without a warehouse. */
            key?: string | null;
            /**
             * Format: int64
             * @description Builds that contributed, counted once each.
             */
            runs: number;
            /** Format: double */
            seconds: number;
            /**
             * Format: double
             * @description None when any of the group's observations had no configured credit price.
             */
            usd?: number | null;
        };
        /** @description The price the dollars of a cost read use, and where it came from. */
        CostPrice: {
            /** @description `config` (`[cost] usd_per_credit`), `rate_sheet`, `list_price` (also when only some) or `mixed`. */
            source: string;
            /** Format: double */
            usd_per_credit: number;
        };
        /** @description A selection's estimated query cost over a period, job by node, with daily series. */
        CostSelectionRecord: {
            /** Format: int64 */
            bytes_scanned: number;
            /** Format: double */
            credits: number;
            /** @description The period's days, `YYYY-MM-DD` in UTC, oldest first; every series follows them. */
            dates: string[];
            /** Format: int32 */
            days: number;
            error?: null | components["schemas"]["SelectionProblem"];
            facts: components["schemas"]["CostFactRecord"][];
            folder_daily: components["schemas"]["CostSeriesRecord"][];
            /**
             * Format: int64
             * @description Of those, builds whose cost is still pending or partial.
             */
            incomplete_runs: number;
            job_daily: components["schemas"]["CostSeriesRecord"][];
            /** Format: double */
            previous_credits: number;
            /** Format: double */
            previous_usd?: number | null;
            price?: null | components["schemas"]["CostPrice"];
            /** Format: int64 */
            queries: number;
            /**
             * Format: int64
             * @description Builds in the period that spent cost on a selected node.
             */
            runs: number;
            /** Format: double */
            seconds: number;
            /** Format: date-time */
            since: string;
            /**
             * Format: uuid
             * @description The publication the selection was evaluated against; None before the first one.
             */
            snapshot_id?: string | null;
            /** Format: double */
            usd?: number | null;
        };
        /** @description Daily series for a batch of groups, in their order. */
        CostSeriesBatch: {
            dates: string[];
            series: number[][];
        };
        /** @description Credits per day of the period, oldest first, for one group. */
        CostSeriesRecord: {
            credits: number[];
            /** @description The job, or top-level folder; None for builds no server run started. */
            key?: string | null;
        };
        /** @description A target's estimated query cost over a window, from the latest observation of each run. */
        CostSummaryRecord: {
            by_job: components["schemas"]["CostGroup"][];
            /** @description Every node that ran, keyed `<resource_type>:<resource_name>` like catalog node keys. */
            by_node: components["schemas"]["CostGroup"][];
            by_warehouse_size: components["schemas"]["CostGroup"][];
            /** Format: double */
            credits: number;
            /**
             * Format: int64
             * @description Runs whose cost is still pending or partial, which a later build re-reads.
             */
            incomplete_runs: number;
            /** Format: int64 */
            runs: number;
            /** Format: date-time */
            since: string;
            /** Format: double */
            usd?: number | null;
            /** @description `config`, `rate_sheet`, `list_price` (also when only some are) or `mixed`; None without dollars. */
            usd_per_credit_source?: string | null;
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
        /** @description Where a declaration was written, relative to the project root. */
        DeclaredAt: {
            /** Format: int32 */
            line: number;
            path: string;
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
        /** @description The node one item of a discovered set became under one `for_each` asset. */
        DiscoveredNodeReadback: {
            /** @description The item's arguments; one that is not a string reads as its JSON text. */
            args: {
                [key: string]: string;
            };
            asset: string;
            /** @description The node key, `asset:<asset>[<item>]`. */
            key: string;
            /** @description The item's ID, or `*` for the placeholder of a set that never succeeded. */
            step: string;
        };
        /** @description The discovered sets of a target's current snapshot; empty when it saved none. */
        DiscoveredReadback: {
            /** Format: int64 */
            generation: number;
            /** Format: date-time */
            published_at: string;
            sets: components["schemas"]["DiscoveredSetReadback"][];
            /** Format: uuid */
            snapshot_id: string;
        };
        /** @description One discovered set of the current snapshot, as a deploy reads it back. */
        DiscoveredSetReadback: {
            /** @description The `for_each` assets that read the set. */
            assets: string[];
            /** Format: int64 */
            check_every_seconds?: number | null;
            digest?: string | null;
            error?: string | null;
            /** Format: date-time */
            fetched_at?: string | null;
            /** Format: int64 */
            items: number;
            /** Format: date-time */
            last_good_at?: string | null;
            name: string;
            /** @description The node each served item became under each of `assets`, or each asset's placeholder. */
            nodes: components["schemas"]["DiscoveredNodeReadback"][];
            status: components["schemas"]["DiscoveredSetStatus"];
            /** @description The tags of the result the set serves (its newest, or its last good one), each as text. */
            tags: {
                [key: string]: string;
            };
        };
        /**
         * @description How a set's newest attempt went.
         * @enum {string}
         */
        DiscoveredSetStatus: "ok" | "failed" | "never";
        /** @description One discovered set as a location's health names it. */
        DiscoveryHealth: {
            /** @description Why the newest attempt failed, secrets masked. */
            error?: string | null;
            /**
             * Format: int64
             * @description How many items the set serves: its newest result, or the last good one after a failure.
             */
            items: number;
            /** Format: date-time */
            last_good_at?: string | null;
            name: string;
            status: components["schemas"]["DiscoveredSetStatus"];
        };
        /** @description A calendar whose pending past-due times a republish dropped, because its previous definition can't be read; it clears at its next successful tick. */
        DroppedCalendarTimes: {
            /** Format: date-time */
            dropped_at: string;
            /** Format: uuid */
            location_id: string;
            location_name: string;
            name: string;
            target: string;
            /** @description The times that were due by the republish but never ticked, oldest first. */
            times: string[];
            /** Format: uuid */
            trigger_id: string;
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
        /** @description A checking-in location whose latest load, plan of what changed, discovery or connection check failed, and what it serves instead. */
        FailingLocation: {
            /** @description What failed, redacted; absent when an older location reported no text. */
            error?: string | null;
            /**
             * Format: date-time
             * @description When the failure was reported.
             */
            failed_at: string;
            /** Format: uuid */
            location_id: string;
            location_name: string;
            serving?: null | components["schemas"]["SnapshotRecord"];
            /** @description `falling_back`, `change_plan_failed`, `discovery_failed` or `degraded`. */
            state: components["schemas"]["LocationState"];
            target: string;
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
        /** @description The control store's installation and schema, as recorded when it was created. */
        InstallationRecord: {
            /** Format: date-time */
            installed_at: string;
            /** Format: date-time */
            schema_applied_at: string;
            /** @description Digest of the schema; a server starts only on the exact schema it creates. */
            schema_checksum: string;
            storage: components["schemas"]["StorageKind"];
            /**
             * Format: int64
             * @description The database's size on disk now: SQLite's main file, or the whole PostgreSQL database.
             */
            store_bytes: number;
        };
        /** @description A service account with a key that is returned only this once; the store keeps only its hash. */
        IssuedServiceKey: {
            account: components["schemas"]["ServiceAccountSummary"];
            key: string;
        };
        /** @description One job or group as declared, for a person who may list the target; free-form setting values are withheld. */
        JobDetailRecord: {
            /**
             * Format: int64
             * @description The job's own `concurrency` setting, models building at once in a run; absent uses the project's.
             */
            concurrency?: number | null;
            /** @description Selected nodes whose failure stops a group. */
            critical_nodes: string[];
            /** @description The cursor type every selected cursor node shares; absent when none has a cursor or types differ. */
            cursor_type?: string | null;
            declared_at?: null | components["schemas"]["DeclaredAt"];
            description: string;
            exclude?: string | null;
            /** @description The job's own `ignore_audit_failures` setting; absent uses the target's, then the project's. */
            ignore_audit_failures?: boolean | null;
            job_key: string;
            kind: components["schemas"]["RunKind"];
            /** @description The groups this job belongs to. */
            member_of: string[];
            /** @description A group's member jobs in run order; empty for a job. */
            members: components["schemas"]["JobMember"][];
            on_failure?: null | components["schemas"]["OnFailure"];
            overlap: components["schemas"]["Overlap"];
            owner?: string | null;
            /** @description Each pool the job takes, by pool name. */
            pools: components["schemas"]["PoolUse"][];
            resolved_concurrency?: null | components["schemas"]["ResolvedConcurrency"];
            select?: string | null;
            selection: string[];
            /** @description Command settings; `vars` and a `defer_*` value that is not a target name show as `[REDACTED]`, and text values pass through secret redaction. */
            settings: {
                [key: string]: unknown;
            };
            /** Format: uuid */
            snapshot_id: string;
            /** @description Empty when the declaration applies to every target. */
            targets: string[];
        };
        JobMember: {
            job_key: string;
            /** @description Node by node, why it waits for each of `waits_for`; a wait with none follows lineage through another member. */
            reasons: components["schemas"]["WaitReason"][];
            /** @description Members this one waits for, from the lineage between their nodes. */
            waits_for: string[];
        };
        /** @description A job or group of the current published snapshot. Settings are left out of person reads. */
        JobRecord: {
            /**
             * Format: int64
             * @description The job's own `concurrency` setting, models building at once in a run; absent uses the project's.
             */
            concurrency?: number | null;
            declared_at?: null | components["schemas"]["DeclaredAt"];
            description: string;
            /** @description The job's own `ignore_audit_failures` setting; absent uses the target's, then the project's. */
            ignore_audit_failures?: boolean | null;
            job_key: string;
            /** @description `job` or `group`; a group's selection is the union of its member jobs' selections. */
            kind: components["schemas"]["RunKind"];
            on_failure?: null | components["schemas"]["OnFailure"];
            overlap: components["schemas"]["Overlap"];
            owner?: string | null;
            /** @description Each pool the job takes, by pool name. */
            pools: components["schemas"]["PoolUse"][];
            resolved_concurrency?: null | components["schemas"]["ResolvedConcurrency"];
            /** @description A job's selectors as written; absent for a group and for snapshots published before they were recorded. */
            select?: string | null;
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
            /**
             * Format: uuid
             * @description A new random (UUID v4) or time-ordered (UUID v7) ID; name-based (v5) IDs are refused as the server derives those.
             */
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
        /** @description Locations report bounded facts, never connection strings; a failed warehouse read carries only its redacted error text. */
        LocationHealth: {
            /** @enum {string} */
            kind: "heartbeat";
        } | {
            /** @description Why the load failed, redacted and bounded; absent from older locations. */
            error?: string | null;
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "compile_failed";
            rules?: null | components["schemas"]["LocationRuleFailure"];
        } | {
            /**
             * Format: int64
             * @description How long the check took; absent from older locations.
             */
            elapsed_millis?: number | null;
            /** Format: int64 */
            generation: number;
            healthy: boolean;
            /** @enum {string} */
            kind: "connection";
            trigger?: null | components["schemas"]["ConnectionCheckTrigger"];
        } | {
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "provider_secrets";
            /**
             * Format: int64
             * @description Unset secrets of providers the target doesn't allow to miss them; above zero is a health problem.
             */
            missing: number;
            /** @description Every provider missing a secret, including those the target allows; names only, never values. */
            providers: components["schemas"]["MissingProviderSecrets"][];
        } | {
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "discovery";
            sets: components["schemas"]["DiscoveryHealth"][];
        } | {
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "change_plan_failed";
            reason: string;
        } | {
            /** Format: int64 */
            generation: number;
            jobs: components["schemas"]["PoolCapWarning"][];
            /** @enum {string} */
            kind: "pool_capped";
        } | {
            /** Format: int64 */
            elapsed_millis: number;
            /** @description A failed read's warehouse error, redacted and bounded; absent otherwise. */
            error?: string | null;
            failure?: null | components["schemas"]["WarehouseSyncFailure"];
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "warehouse_sync";
            location: components["schemas"]["WarehouseStateLocation"];
            /**
             * Format: int64
             * @description On the report of a table's last delivered page, the records the server then mirrored of that table across the target's state locations.
             */
            mirrored_records?: number | null;
            /**
             * Format: date-time
             * @description On the report that completes or stops a sync, when the next safety sync is due unless a run ends first; absent otherwise, where no timed sync follows, and from older locations.
             */
            next_sync_at?: string | null;
            /**
             * Format: int64
             * @description Rows, or statistics relations, the failed read asked for.
             */
            page_limit?: number | null;
            progress?: null | components["schemas"]["WarehouseSyncProgress"];
            read_failure?: null | components["schemas"]["WarehouseReadFailure"];
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
        /** @description Configured rules that failed a location's load: how many findings, and the first of them. */
        LocationRuleFailure: {
            /** Format: int64 */
            finding_count: number;
            /** @description At most `MAX_REPORTED_RULE_FINDINGS`, in the rules' order. */
            findings: components["schemas"]["LocationRuleFinding"][];
        };
        /** @description One rule finding as `sqb compile` prints it. */
        LocationRuleFinding: {
            code: string;
            /** Format: int64 */
            line?: number | null;
            message: string;
            path?: string | null;
        };
        /**
         * @description One target's state from its newest health observations, as the web UI shows it.
         * @enum {string}
         */
        LocationState: "starting" | "healthy" | "degraded" | "falling_back" | "change_plan_failed" | "missing_secrets" | "discovery_failed" | "broken";
        /** @description Only named location operations cross this boundary, never arbitrary SQL or Python. */
        LocationWork: {
            /** @enum {string} */
            kind: "check_connection";
        } | {
            /** @enum {string} */
            kind: "sync_warehouse";
        } | {
            evaluation: Record<string, never>;
            /** @enum {string} */
            kind: "evaluate_sensor";
        } | {
            command: string[];
            /** @description Plan again even when the location kept a plan for the same request. */
            fresh: boolean;
            /** @description The settings the command gives, `warehouse` included though a plan never takes it; they replace the pinned jobs' own. */
            given?: string[];
            /** @description The jobs the command names, pinned as an ad-hoc build of it pins them. */
            jobs?: {
                [key: string]: components["schemas"]["PinnedJob"];
            };
            /** @enum {string} */
            kind: "plan";
            selection: string[];
            settings: Record<string, never>;
        } | {
            /** @enum {string} */
            kind: "republish";
            /** @description The name of whoever asked, which the location prints with the request's reason. */
            requested_by: string;
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
            outcome: components["schemas"]["SensorFunctionOutcome"];
        } | {
            /** @description Model diff text, which the document never carries; stored apart, never read back here. */
            diffs?: {
                [key: string]: components["schemas"]["PlanModelDiff"];
            };
            document: Record<string, never>;
            /** @enum {string} */
            kind: "plan";
        } | {
            /** @enum {string} */
            kind: "sync_started";
        } | {
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "published";
            /** Format: uuid */
            snapshot_id: string;
        } | {
            error: string;
            /** Format: int64 */
            generation: number;
            /** @enum {string} */
            kind: "publish_failed";
        } | {
            /** @enum {string} */
            kind: "superseded";
        };
        LocationWorkPoll: {
            /** @description Which work this poll takes. */
            scope: components["schemas"]["LocationWorkScope"];
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
         * @description Which work a location takes in one poll.
         * @enum {string}
         */
        LocationWorkScope: "all" | "republish_only";
        /**
         * @description How people sign in to this server.
         * @enum {string}
         */
        LoginMode: "password" | "trusted_proxy" | "oidc" | "disabled" | "keys";
        /** @description How a server without a session is signed in to, answered before sign-in only where people can't. */
        LoginModeAnswer: {
            login_mode: components["schemas"]["LoginMode"];
        };
        /** @description A running calendar schedule whose last date is at most a day away, or which has none left. */
        LowCalendar: {
            /**
             * Format: date-time
             * @description The calendar's last date; absent when none are left.
             */
            horizon_end?: string | null;
            /** Format: uuid */
            location_id: string;
            location_name: string;
            name: string;
            /**
             * Format: date-time
             * @description When the location next works out the calendar's dates.
             */
            refresh_at: string;
            target: string;
            /** Format: uuid */
            trigger_id: string;
        };
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
            /** @description The table's read ends with this page, so the reply counts the records mirrored of it. */
            last_page?: boolean;
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
        /** @description A provider whose required secrets are unset: what is missing and which nodes need it, never a value. */
        MissingProviderSecrets: {
            /** @description The target's `allow_missing_secrets` lists the provider, so its location shows a note, not a problem. */
            allowed: boolean;
            /** @description Snapshot node keys that run the provider, such as `source:raw_payments`. */
            nodes: string[];
            /** @description The provider's file, relative to the project directory. */
            path: string;
            provider: string;
            settings: components["schemas"]["MissingProviderSetting"][];
        };
        /** @description One unset required setting: the field, the variable it reads and the field's line in the provider file. */
        MissingProviderSetting: {
            field: string;
            /** Format: int64 */
            line?: number | null;
            variable: string;
        };
        MutationResponse: {
            applied: boolean;
        };
        /** @description A service account to add: its name, what uses it, and its built-in role everywhere. */
        NewServiceAccount: {
            description?: string | null;
            name: string;
            role: components["schemas"]["BuiltinRole"];
        };
        /**
         * @description Where a node stands: built and current, or why not.
         * @enum {string}
         */
        NodeBuildStatus: "current" | "changed" | "behind" | "failed" | "not_built" | "source";
        /** @description One build's estimated query cost of one node, across every warehouse it used. */
        NodeCostRecord: {
            /** Format: double */
            credits: number;
            orchestration_run_id?: string | null;
            /** Format: int64 */
            query_count: number;
            run_id: string;
            /** Format: date-time */
            run_started_at: string;
            /** Format: double */
            seconds: number;
            /** @description The collection status of the whole build. */
            status: string;
            /** Format: double */
            usd?: number | null;
            /** @description Where the build's price per credit came from: `config`, `rate_sheet` or `list_price`. */
            usd_per_credit_source?: string | null;
        };
        /** @enum {string} */
        NodeEventKind: "success" | "failed" | "recovered";
        /**
         * @description The level of a line a node logged with `ctx.log`.
         * @enum {string}
         */
        NodeLogLevel: "info" | "warning" | "error";
        /** @description One line a node logged with `ctx.log`. */
        NodeLogLine: {
            level: string;
            /** Format: date-time */
            logged_at: string;
            message: string;
            node_key: string;
        };
        /** @enum {string} */
        NodeOutcome: "success" | "warn" | "failed" | "skipped" | "cancelled" | "not_run";
        /** @description How much one node printed: its lines, numbered across both streams, and its bytes. */
        NodeOutputSummary: {
            /** Format: int64 */
            line_count: number;
            /** @description The node key, or `run` for lines printed outside any node. */
            node_key: string;
            /** Format: int64 */
            stderr_lines: number;
            /** Format: int64 */
            stdout_lines: number;
            /** Format: int64 */
            text_bytes: number;
        };
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
            /** @description Each placed column's defining expression as a `[start, end)` byte range of `compiled_sql`. */
            expression_ranges?: {
                [key: string]: number[];
            };
            node_key: string;
            raw_sql?: string | null;
            /** Format: uuid */
            snapshot_id: string;
        };
        /** @description The state fields of a `CatalogNodeRecord`, with the same meaning, and the status every page shows. */
        NodeState: {
            checks: components["schemas"]["CatalogNodeChecks"];
            definition_changed: boolean;
            last_build?: null | components["schemas"]["CatalogNodeBuild"];
            last_outcome?: null | components["schemas"]["CatalogNodeOutcome"];
            /** @description The node's one build status, which pages show as it is. */
            status?: components["schemas"]["NodeBuildStatus"];
            /**
             * Format: date-time
             * @description When the status was reached: the last build, or the last run for a node without a table.
             */
            status_at?: string | null;
        };
        /** @description Every current node's state at one version; `snapshot_id` is null before the first publication. */
        NodeStatesRecord: {
            changed_build?: null | components["schemas"]["ChangedBuild"];
            /** Format: uuid */
            snapshot_id?: string | null;
            states: {
                [key: string]: components["schemas"]["NodeState"];
            };
            /** Format: int64 */
            version: number;
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
        /** @description A failed run reruns from failure up to `retries` times, each `retry_delay_seconds` after the last ended. */
        OnFailure: {
            /** Format: int32 */
            retries: number;
            /** Format: int32 */
            retry_delay_seconds: number;
        };
        /** @description What a worker printed in one batch window, a segment per node and stream in the order printed. */
        OutputBatch: {
            /**
             * Format: int64
             * @description The worker's batch number within its claim, from 0; a retried batch is stored once.
             */
            batch: number;
            segments: components["schemas"]["OutputSegment"][];
        };
        /** @description Whether the server stored a batch; a retry of a stored batch is not stored again. */
        OutputBatchReceipt: {
            stored: boolean;
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
        /**
         * @description How a printed line reads: as an error or as a warning.
         * @enum {string}
         */
        OutputLevel: "error" | "warning";
        /** @description One stored line; lines a size cap left out just before it are counted on it. */
        OutputLine: {
            /**
             * Format: date-time
             * @description When the server received the batch holding the line, within a few seconds of its printing.
             */
            at: string;
            level?: null | components["schemas"]["OutputLevel"];
            /** @description The node that printed it, or `run`. */
            node_key: string;
            /** Format: int64 */
            number: number;
            /** Format: int64 */
            omitted_before?: number;
            stream: components["schemas"]["OutputStream"];
            text: string;
        };
        /** @description A line that looks like an error, such as a traceback, or a warning; each stream keeps at most 10,000. */
        OutputMark: {
            level: components["schemas"]["OutputLevel"];
            /**
             * Format: int64
             * @description The line in the node's numbering.
             */
            line: number;
            stream: components["schemas"]["OutputStream"];
            /** @description The line's text, at most 300 bytes. */
            text: string;
        };
        /** @description Rows covering lines `from` up to `to`, and where the pages before and after start. */
        OutputPage: {
            /**
             * Format: int64
             * @description The first line the rows cover.
             */
            from: number;
            /**
             * Format: int64
             * @description The highest line number of the node, or of the run.
             */
            line_count: number;
            /**
             * Format: int64
             * @description `from` for the next page, when there are later lines.
             */
            next?: number | null;
            /**
             * Format: int64
             * @description `before` for the previous page, when there are earlier lines.
             */
            previous?: number | null;
            rows: components["schemas"]["OutputRow"][];
            /**
             * Format: int64
             * @description The line after the last the rows cover.
             */
            to: number;
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
        /** @description A line, or a run of lines that are the same but for their numbers, folded into one row. */
        OutputRow: {
            /** @enum {string} */
            kind: "line";
            line: components["schemas"]["OutputLine"];
        } | {
            /** Format: int64 */
            count: number;
            /** Format: int64 */
            from: number;
            /** @enum {string} */
            kind: "similar";
            sample: components["schemas"]["OutputLine"];
        };
        /** @description The next match; across the whole output, how many match and which this is, when the output was small enough to scan; or where to continue. */
        OutputSearch: {
            /**
             * Format: int64
             * @description `from` for the next search in the same direction, when this one stopped before finding a match.
             */
            continue_from?: number | null;
            /** Format: int64 */
            count?: number | null;
            /** Format: int64 */
            line?: number | null;
            /** Format: int64 */
            rank?: number | null;
        };
        /** @description Complete lines one node printed to one stream, each ending in a newline. */
        OutputSegment: {
            /**
             * Format: int64
             * @description Bytes the worker dropped just before these lines because it could not send them fast enough.
             */
            dropped_bytes?: number;
            /** @description The node that printed the lines, keyed `<kind>:<name>`; `None` for lines printed outside a node. */
            node_key?: string | null;
            /**
             * Format: date-time
             * @description When the worker read these lines from the command's output, to the millisecond.
             */
            read_at: string;
            stream: components["schemas"]["OutputStream"];
            text: string;
        };
        /**
         * @description One process stream.
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
        /**
         * @description What a schedule tick or sensor request does while a run of the same job is active; people's requests always queue.
         * @enum {string}
         */
        Overlap: "queue" | "skip";
        PasswordChangeRequest: {
            current_password: string;
            new_password: string;
        };
        PasswordLoginRequest: {
            password: string;
            username: string;
        };
        /** @description A person to add who signs in with a password: their sign-in name, first password and built-in role everywhere, if any. */
        PasswordPersonRequest: {
            name: string;
            password: string;
            role?: null | components["schemas"]["BuiltinRole"];
        };
        PasswordSessionResponse: {
            csrf_token: string;
            identity: components["schemas"]["SessionIdentity"];
        };
        /** @description The past runs read, or why one can't be: too short, unknown or ambiguous. */
        PastRunsRead: {
            error?: string | null;
            /** @description Each run by the ID as given; empty when `error` is set. */
            runs: components["schemas"]["BTreeMap"];
        };
        /** @description The person's name, the actions allowed server-wide, and the location targets they can view. */
        PersonAccess: {
            actions: components["schemas"]["Action"][];
            locations: components["schemas"]["LocationAccess"][];
            name: string;
            /** Format: uuid */
            principal_id: string;
        };
        /** @description The person just added. */
        PersonCreated: {
            /** Format: uuid */
            principal_id: string;
        };
        /** @description A target's health as people read it: its state, computed only here, and the newest observations it was computed from. */
        PersonLocationHealth: {
            /** @description Newest first, with this server's pool-cap warning on the current snapshot. */
            observations: components["schemas"]["LocationHealthObservation"][];
            state: components["schemas"]["LocationState"];
        };
        /** @description A stored platform event a person may see; the location target is absent for server-wide events. */
        PersonPlatformEvent: {
            event: components["schemas"]["PlatformEvent"];
            /** Format: uuid */
            event_id: string;
            /** Format: uuid */
            location_id?: string | null;
            /** Format: date-time */
            occurred_at: string;
            /** Format: int64 */
            storage_id: number;
            target?: string | null;
        };
        /** @description One scan of stored platform events, keeping those the person may see. */
        PersonPlatformEventPage: {
            /**
             * Format: int64
             * @description The `before` that continues the scan, absent once no older events remain; a page can be empty and still continue.
             */
            before?: number | null;
            events: components["schemas"]["PersonPlatformEvent"][];
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
        /** @description The event position a condition reads from, and a polling sensor's own value after secret redaction; the value needs view.evidence. */
        PersonSensorCursor: {
            custom?: string | null;
            /** Format: int64 */
            event_storage_id: number;
        };
        PersonSensorEvaluation: {
            abandoned_because?: null | components["schemas"]["AbandonReason"];
            cursor_after?: null | components["schemas"]["PersonSensorCursor"];
            cursor_before: components["schemas"]["PersonSensorCursor"];
            definition_fingerprint: string;
            /** @description The traceback or refusal of a failed tick, redacted; needs view.evidence. */
            error?: string | null;
            /** Format: uuid */
            evaluation_id: string;
            /** @description The event the function was called with; absent for a polling sensor. */
            event_id?: string | null;
            event_summary?: string | null;
            /** @description The failure code of a failed tick. */
            failure_code?: string | null;
            /** Format: date-time */
            finished_at?: string | null;
            /** @description What the function did: `handled`, `skipped`, `started`, `failed`, `timed_out`, `abandoned`, or `running` while in flight. */
            outcome: string;
            /** @description The latest phase: `requested` alone means the tick is in flight; otherwise `completed`, `failed`, `timed_out` or `abandoned`. */
            phase: string;
            /** Format: date-time */
            requested_at: string;
            /** @description The runs the tick started. */
            run_ids: string[];
            /** @description Why the function skipped, redacted; needs view.evidence. */
            skip_reason?: string | null;
            /** @description The operator who skipped the tick's event, so the function wasn't called. */
            skipped_by?: string | null;
            /** @description For each requested run the job's `skip` overlap dropped, the run that was active. */
            skipped_for: string[];
            /** @description Why the tick's details are missing: an earlier release recorded them in a form this one can't read. */
            unreadable?: string | null;
        };
        /** @description A run key is redacted and withheld like the custom cursor it usually derives from: empty without view.evidence. */
        PersonSensorRunRequest: {
            /** Format: uuid */
            evaluation_id: string;
            /** Format: date-time */
            queued_at: string;
            /** Format: uuid */
            run_id: string;
            run_key: string;
            status: components["schemas"]["RunStatus"];
        };
        /** @description A sensor's committed position and recent checks, for a person who may list the target. */
        PersonSensorState: {
            cursor?: null | components["schemas"]["PersonSensorCursor"];
            /** @description Fingerprint of the published definition; a check with another one ran older code. */
            definition_fingerprint: string;
            /** @description Newest first. */
            evaluations: components["schemas"]["PersonSensorEvaluation"][];
            failing?: null | components["schemas"]["SensorFailing"];
            /** @description Runs the sensor requested, by run key, newest first. */
            run_requests: components["schemas"]["PersonSensorRunRequest"][];
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
            sensor_failing?: null | components["schemas"]["SensorFailing"];
            /** @description A running schedule's next due times from `next_due_at` on, at most five; empty otherwise. */
            upcoming: string[];
        };
        /** @description A person asks a location for one named read-only operation, such as a connection test. */
        PersonWorkRequest: {
            reason: string;
            /** Format: uuid */
            request_id: string;
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
        /** @description A job an ad-hoc selection names: the frozen nodes it gives its settings to, and those settings. */
        PinnedJob: {
            selection: string[];
            settings: {
                [key: string]: unknown;
            };
        };
        /** @description A planned model's diff text, stored beside its plan document. */
        PlanDiffRead: components["schemas"]["PlanModelDiff"] & {
            model: string;
        };
        /** @description One planned model's diff text, as `sqb plan` prints it. */
        PlanModelDiff: {
            config_diff?: string[] | null;
            query_diff?: string[] | null;
        };
        /** @description A person's build command to plan on the location; the server freezes it like an ad-hoc run. */
        PlanRequest: {
            command: string[];
            /** @description Plan again instead of returning the plan the location kept for the same request. */
            fresh: boolean;
            /** Format: uuid */
            request_id: string;
        };
        /** @description A page of a plan section with the `--changed` runs, warnings and cursor window starts of its models. */
        PlanRows: {
            cursor_window_starts: {
                [key: string]: string;
            };
            /** @description Model rows carry `diff_stats` in place of their diff text, which `plans/{request_id}/diff` serves. */
            rows: Record<string, never>[];
            runs: Record<string, never>[];
            /** @description Rows in the section, filter or not. */
            section_total: number;
            /** @description Rows matching the name filter; the section's size without one. */
            total: number;
            warnings: Record<string, never>[];
        };
        /** @description A plan request's state with its summary: changed models in full, large sections as counts to page. */
        PlanSummaryRead: {
            /** Format: date-time */
            expires_at: string;
            outcome?: null | components["schemas"]["LocationWorkOutcome"];
            timings?: null | components["schemas"]["PlanTimings"];
        };
        /** @description One planning step and its wall time. */
        PlanTimingStep: {
            name: string;
            /** Format: double */
            seconds: number;
        };
        /** @description How long one plan took: its steps in pipeline order with `Other` last, and what it read. */
        PlanTimings: {
            /** @description `synced_state` for a preview over the synced warehouse state; `live` when it read the warehouse. */
            planned_from: string;
            /**
             * Format: date-time
             * @description When the plan the location kept, and answered with again, was made; `None` for a new plan.
             */
            reused_from?: string | null;
            steps: components["schemas"]["PlanTimingStep"][];
            /** Format: double */
            total_seconds: number;
        };
        PlatformEvent: {
            /** @enum {string} */
            kind: "heartbeat";
            /** Format: uuid */
            replica_id: string;
            /**
             * Format: int64
             * @description The control store's database size on disk when the replica checked in.
             */
            store_bytes: number;
        } | {
            /** @enum {string} */
            kind: "scheduler_no_leader";
        } | {
            /** @enum {string} */
            kind: "tick_overdue";
            /** Format: date-time */
            scheduled_for: string;
            /** Format: uuid */
            trigger_id: string;
        } | {
            /** Format: date-time */
            horizon_end?: string | null;
            /** @enum {string} */
            kind: "calendar_horizon_low";
            /** Format: date-time */
            refresh_at: string;
            /** Format: uuid */
            trigger_id: string;
        } | {
            /** @enum {string} */
            kind: "location_broken";
            /** Format: date-time */
            last_seen_at: string;
            /** Format: uuid */
            location_id: string;
            target: string;
        } | {
            error?: string | null;
            /** Format: date-time */
            failed_at: string;
            /** @enum {string} */
            kind: "location_failing";
            /** Format: uuid */
            location_id: string;
            /** Format: int64 */
            serving_generation?: number | null;
            state: components["schemas"]["LocationState"];
            target: string;
        } | {
            /** Format: uuid */
            claim_id: string;
            code: string;
            /** @enum {string} */
            kind: "launch_failed";
            /** Format: uuid */
            run_id: string;
        } | (components["schemas"]["RunFailureEvent"] & {
            /** @enum {string} */
            kind: "run_failed";
        }) | {
            /** @enum {string} */
            kind: "trigger_skipped";
            reason: string;
            /** Format: uuid */
            snapshot_id: string;
            /** Format: uuid */
            trigger_id: string;
        } | {
            /** Format: uuid */
            evaluation_id: string;
            /** @enum {string} */
            kind: "sensor_rechecked";
            /** Format: uuid */
            snapshot_id: string;
            /** Format: uuid */
            trigger_id: string;
        } | {
            /** @enum {string} */
            kind: "calendar_times_dropped";
            /** Format: uuid */
            snapshot_id: string;
            times: string[];
            /** Format: uuid */
            trigger_id: string;
        };
        /** @enum {string} */
        PlatformEventKind: "heartbeat" | "scheduler_no_leader" | "tick_overdue" | "calendar_horizon_low" | "location_broken" | "location_failing" | "launch_failed" | "run_failed" | "trigger_skipped" | "sensor_rechecked" | "calendar_times_dropped";
        /** @description Problems derived from live state when read; nothing here is stored. */
        PlatformProblems: {
            broken_locations: components["schemas"]["BrokenLocation"][];
            /** @description Calendars whose pending times a republish dropped, until their next successful tick. */
            dropped_calendar_times?: components["schemas"]["DroppedCalendarTimes"][];
            /** @description Checking-in locations whose latest load, plan of what changed, discovery or connection check failed. */
            failing_locations?: components["schemas"]["FailingLocation"][];
            /** @description Running calendar schedules whose dates run out within a day. */
            low_calendars?: components["schemas"]["LowCalendar"][];
            overdue_ticks: components["schemas"]["OverdueTick"][];
            /** @description Sensors checked again from scratch because their last check can't be read, until a check completes. */
            rechecked_sensors?: components["schemas"]["RecheckedSensor"][];
            scheduler_leader: boolean;
            /**
             * Format: date-time
             * @description When the last scheduler lease ran out, or will run out; absent until a server first leads.
             */
            scheduler_lease_expires_at?: string | null;
            /** @description Triggers skipped because their stored job or pinned snapshot can't be read, until their location publishes again. */
            skipped_triggers?: components["schemas"]["SkippedTrigger"][];
        };
        /** @description A job or group asking for more of a pool than this server's cap, so it runs alone in that pool. */
        PoolCapWarning: {
            /** Format: int32 */
            amount: number;
            /** Format: int32 */
            cap: number;
            job_key: string;
            kind: components["schemas"]["RunKind"];
            /** @description Such as "job X takes 3 of pool warehouse_heavy; this server caps it at 1, so it runs alone in that pool". */
            message: string;
            pool: string;
        };
        /** @description How many slots of a pool a job takes, and the pool's limit on the job's target. */
        PoolSlots: {
            /** Format: int32 */
            amount: number;
            /** Format: int32 */
            limit: number;
        };
        /** @description A pool a job takes: its slots, the pool's limit, and the other jobs and groups taking it. */
        PoolUse: {
            /**
             * Format: int32
             * @description The slots the job asks for.
             */
            amount: number;
            /**
             * Format: int32
             * @description This server's `pool_caps` entry for the location and target; absent when it sets none.
             */
            cap?: number | null;
            /** @description The job asks for more slots than `server_limit`, so on this server it takes the whole pool. */
            clamped: boolean;
            /**
             * Format: int32
             * @description The project's limit on the target.
             */
            limit: number;
            pool: string;
            /**
             * Format: int32
             * @description The limit on this server: the project's limit, lowered to the cap.
             */
            server_limit: number;
            shared_with: string[];
        };
        /** @description What the mirror knows as of `synced_at`; `None` when a required table never synced cleanly. */
        PreviewCopy: {
            /** @description Every mirrored consumption row when asked for; omitted otherwise, as older locations reject it. */
            consumption?: components["schemas"]["WarehouseConsumptionRecord"][];
            fingerprints: components["schemas"]["WarehouseFingerprintRecord"][];
            latest_runs: components["schemas"]["PreviewLatestRun"][];
            observations: components["schemas"]["WarehouseObservationRecord"][];
            relations: components["schemas"]["WarehouseCatalogRecord"][];
            /** Format: date-time */
            synced_at?: string | null;
            /** @description Set when the mirror is still at the read's known version: the records are left out. */
            unchanged?: boolean;
            /** @description The mirror version these records were read at. */
            version: string;
        };
        /** @description A location asks for the mirrored copy a plan preview plans from, as its fenced reader. */
        PreviewCopyRead: {
            /** @description Whether the copy carries consumption history. */
            consumption?: boolean;
            /** Format: int64 */
            generation: number;
            /** @description The mirror version of the copy the location holds; an unchanged mirror returns no records. */
            known_version?: string | null;
            /** Format: uuid */
            session_id: string;
        };
        /** @description The newest successful build of a node, which a `cursor_max` observation must come from. */
        PreviewLatestRun: {
            node_name: string;
            node_type: string;
            run_id: string;
            state_database?: string | null;
            state_schema: string;
        };
        /**
         * @description A principal signs in as a person, or calls with an API key as a service account.
         * @enum {string}
         */
        PrincipalKind: "person" | "service";
        /** @description Code that injects a provider: a node (`loader`, `asset`, `check`, `hook` or `materialization`, keyed `kind:name`), an `event_exporter` or a `sink`. */
        ProviderConsumer: {
            kind: string;
            name: string;
        };
        /** @description A provider as its location published it, from the class alone: never a setting value. */
        ProviderRecord: {
            /** @description The class as `module.qualname`. */
            class: string;
            /** @description The class docstring. */
            description: string;
            /** @description Each of those jobs with the provider's nodes it selects, in node order. */
            job_nodes?: {
                [key: string]: string[];
            };
            /** @description Jobs whose selection includes one of those nodes. */
            jobs?: string[];
            name: string;
            /** @description Snapshot nodes that run that code: Python nodes, models calling a hook or using a materialization, and sources a loader fills. */
            nodes?: string[];
            /** @description The file declaring the class, relative to the project. */
            path: string;
            settings: components["schemas"]["ProviderSetting"][];
            /** @description Code that injects the provider, in discovery order. */
            used_by: components["schemas"]["ProviderConsumer"][];
        };
        /** @description One declared setting field; its value stays in the location's environment. */
        ProviderSetting: {
            description?: string | null;
            /** @description The environment variable the setting is read from. */
            env: string;
            /** @description Whether this required secret is unset in the location's environment. */
            missing?: boolean;
            name: string;
            required: boolean;
            /** @description Whether the annotation is a pydantic secret type. */
            secret: boolean;
            /** @description The annotation as written, such as `str` or `SecretStr | None`. */
            type: string;
        };
        /** @description Providers the current snapshot declares; empty before a publish or when the project has none. */
        ProvidersRead: {
            providers: components["schemas"]["ProviderRecord"][];
            /** Format: uuid */
            snapshot_id?: string | null;
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
            connection?: null | components["schemas"]["ConnectionDetails"];
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
        /** @description Why the scheduler cannot claim a queued run yet. Runs outside the listed location and target are counted, never named. */
        QueueWait: {
            /** @enum {string} */
            kind: "group_parent";
            /** Format: uuid */
            run_id: string;
        } | {
            /**
             * Format: int32
             * @description The upstream job's attempt when the group is retrying it; absent for its first attempt.
             */
            attempt?: number | null;
            job_key: string;
            /** @enum {string} */
            kind: "upstream_job";
            /** Format: uuid */
            run_id: string;
        } | {
            /** @enum {string} */
            kind: "job_running";
            /** Format: uuid */
            run_id: string;
        } | {
            /** @enum {string} */
            kind: "write_claim";
            node_key: string;
            /** Format: int32 */
            other_nodes: number;
            /** Format: uuid */
            run_id?: string | null;
        } | {
            holders: string[];
            /** @enum {string} */
            kind: "pool_full";
            pool: string;
        } | {
            /** @enum {string} */
            kind: "earlier_run";
            pool: string;
            /** Format: uuid */
            run_id?: string | null;
        } | {
            /** Format: int32 */
            attempt: number;
            /** Format: date-time */
            due_at: string;
            /** @enum {string} */
            kind: "retry_delay";
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
        /** @description A sensor whose last check, recorded by an earlier release, can't be read, so it is checked again from scratch; it clears once a check completes. */
        RecheckedSensor: {
            /** Format: uuid */
            location_id: string;
            location_name: string;
            name: string;
            /** Format: date-time */
            rechecked_at: string;
            target: string;
            /** Format: uuid */
            trigger_id: string;
        };
        /**
         * @description Where a republish request stands.
         * @enum {string}
         */
        RepublishState: "pending" | "published" | "failed" | "expired";
        /** @description A republish request and its result once the location has one. */
        RepublishStatus: {
            /** Format: date-time */
            completed_at?: string | null;
            error?: string | null;
            /** Format: date-time */
            expires_at: string;
            /** Format: int64 */
            generation?: number | null;
            /** Format: uuid */
            location_id: string;
            reason: string;
            /** Format: uuid */
            request_id: string;
            /** Format: date-time */
            requested_at: string;
            requested_by: string;
            /** Format: uuid */
            snapshot_id?: string | null;
            state: components["schemas"]["RepublishState"];
            target: string;
        };
        /** @description A run a sensor's function asked for; the key defaults to the event ID and the job. */
        RequestedRun: {
            job: string;
            run_key: string;
        };
        /** @description A retry derives its execution scope from a terminal run, never from caller selectors. */
        RerunRequest: {
            from_failure: boolean;
            /** @description Rerun only these nodes of the source run; each must be in its selection. */
            nodes?: string[];
            /** Format: int64 */
            priority: number;
            reason: string;
            /**
             * Format: uuid
             * @description A new random (UUID v4) or time-ordered (UUID v7) ID; name-based (v5) IDs are refused as the server derives those.
             */
            run_id: string;
            /** Format: uuid */
            source_run_id: string;
        };
        /** @description The most of a job's nodes a run of it builds at once on its target. */
        ResolvedConcurrency: {
            /** Format: int64 */
            limit: number;
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
        /** @description A run whose latest acknowledgement event marks it handled. */
        RunAcknowledgement: {
            /** Format: date-time */
            acknowledged_at: string;
            /** @description The person's name, or the audit actor when the person no longer exists. */
            acknowledged_by: string;
            /** Format: uuid */
            run_id: string;
        };
        /** @description An error-severity audit failure in one run: its name, the warehouse's SQLSTATE and redacted error text, or its failing rows. */
        RunAuditFailure: {
            /**
             * Format: int64
             * @description The rows that failed it, when its query ran.
             */
            failing_rows?: number | null;
            /** @description The warehouse's error when the query could not run, redacted and bounded like a node's. */
            message?: string | null;
            name: string;
            /** @description The node it checks; null for an end-of-build singular audit. */
            node_key?: string | null;
            /** @description The warehouse's SQLSTATE when the audit's query could not run, such as `42S02`. */
            sqlstate?: string | null;
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
            /** @enum {string} */
            kind: "node_progress";
            node_key: string;
            /** Format: int64 */
            rows_so_far: number;
        } | {
            /** @enum {string} */
            kind: "node_log";
            level: components["schemas"]["NodeLogLevel"];
            message: string;
            node_key: string;
        } | {
            error_code?: string | null;
            /** @description Redacted and bounded on the way in, whoever sends it; absent from older workers. */
            error_message?: string | null;
            /** @enum {string} */
            kind: "node_finished";
            /** @description Why a node that ran changed nothing. */
            no_change_reason?: string | null;
            node_key: string;
            outcome: components["schemas"]["NodeOutcome"];
            /** @description Where a failed model failed, such as `staging` or `post_hook`; a short lowercase token or absent. */
            phase?: string | null;
            /** Format: int64 */
            rows_affected?: number | null;
            skipped_because_of?: string | null;
            /**
             * Format: int64
             * @description The rows a fully rebuilt table holds, where the warehouse's metadata count is exact.
             */
            table_rows?: number | null;
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
            kind: "compiled";
            node_hashes: {
                [key: string]: string;
            };
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
        } | {
            /** Format: int32 */
            count: number;
            /** @enum {string} */
            kind: "audit_failures_ignored";
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
        RunEventKind: "completed" | "success" | "warn" | "failed" | "cancelled" | "skipped";
        /** @description Recall event metadata of a run. Payloads are left out: they can carry SQL, request settings and error text. */
        RunEventRecord: {
            /** Format: int64 */
            affected_rows?: number | null;
            check_binding_key?: string | null;
            /** @description For a check's result: its name as declared, its ID and this result's, and the rows it found; never its SQL or rows. */
            check_name?: string | null;
            check_result_id?: string | null;
            /** Format: int64 */
            check_rows?: number | null;
            /** Format: double */
            duration_ms?: number | null;
            error_code?: string | null;
            error_type?: string | null;
            event_id: string;
            event_type: string;
            invocation_id: string;
            /** @description A summary of what an event names, such as a member retry or the command a worker started; never error or SQL text. */
            message?: string | null;
            occurred_at: string;
            outcome?: string | null;
            /** @description For a phase of a worker's command, such as `compiled` or `planned`: what it counted, what it is about and its sub-steps. */
            phase?: string | null;
            /** Format: int64 */
            phase_count?: number | null;
            phase_detail?: string | null;
            phase_name?: string | null;
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
            /** @description The discovered sets the run's snapshot saved, which the worker compiles from. */
            discovered: Record<string, never>;
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
        /** @description Why a run failed outside any node: where, the error's type or code, and its redacted, bounded text. */
        RunFailureCause: {
            code?: string | null;
            message?: string | null;
            phase: components["schemas"]["RunFailurePhase"];
        };
        /** @description What a `run_failed` event says: the run, what started it, when, what failed and why, with UI links. */
        RunFailureEvent: {
            /** Format: int64 */
            duration_ms?: number | null;
            /** Format: date-time */
            ended_at?: string | null;
            error?: null | components["schemas"]["RunFailureCause"];
            /** Format: int64 */
            failed_audit_count: number;
            /** @description The first error-severity audit failures by name. */
            failed_audits: components["schemas"]["RunAuditFailure"][];
            /** Format: int64 */
            failed_node_count: number;
            /** @description The first failed nodes by key, with redacted and bounded error text. */
            failed_nodes: components["schemas"]["RunNodeFailure"][];
            job_key?: string | null;
            /** Format: uuid */
            location_id: string;
            outcome: components["schemas"]["RunOutcome"];
            /** Format: uuid */
            run_id: string;
            run_kind: components["schemas"]["RunKind"];
            /** Format: int64 */
            skipped_node_count: number;
            /** @description The first skipped nodes by key. */
            skipped_nodes: string[];
            /** Format: date-time */
            started_at?: string | null;
            /** Format: int64 */
            succeeded_node_count: number;
            target: string;
            trigger: components["schemas"]["RunTrigger"];
            /** @description The run's UI page; null when the server has no origin. */
            url?: string | null;
        };
        /**
         * @description Where a run failed outside any node.
         * @enum {string}
         */
        RunFailurePhase: "launch" | "worker_start" | "load" | "planning" | "build";
        /** @description One error-severity audit failure a run or one of its current members let through; never its SQL or evidence. */
        RunIgnoredAuditFailure: {
            attached_column_name?: string | null;
            attached_target_kind?: string | null;
            attached_target_name?: string | null;
            audit_name: string;
            /** @description The check's identity, as the checks endpoints name it. */
            binding_key?: string | null;
            job_key?: string | null;
            occurred_at: string;
            /**
             * Format: uuid
             * @description The run that evaluated the audit: the run itself or one of its group members.
             */
            run_id: string;
            severity: string;
            /** Format: int64 */
            violation_count?: number | null;
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
            /** @description The node's output in the UI; null when the server has no origin. */
            logs_url?: string | null;
            /** @description Null when the worker reported none or the run's history has been pruned. */
            message?: string | null;
            node_key: string;
            /** @description Relative to the project root, with `/` separators; null when the run's snapshot is gone. */
            path?: string | null;
            /** @description Where a model failed, such as `staging`, `audit` or `post_hook`; null for other nodes or when not reported. */
            phase?: string | null;
            /** @description The node in the run's UI page; null when the server has no origin. */
            url?: string | null;
        };
        RunNodeRecord: {
            /** Format: date-time */
            ended_at?: string | null;
            error_code?: string | null;
            /** @description The node's materialization in the run's snapshot, such as `table`, `incremental` or `view`. */
            materialization?: string | null;
            /** @description For a node that failed unstarted because a provider was missing secrets, its redacted message naming the provider, variables and places looked; never other error text. */
            missing_secret?: string | null;
            /** @description Why a node that ran changed nothing, such as `unchanged since its last build`. */
            no_change_reason?: string | null;
            node_key: string;
            /**
             * Format: int64
             * @description Rows a statement inserted, merged or deleted, as the warehouse reported them.
             */
            rows_affected?: number | null;
            /**
             * Format: int64
             * @description Rows written so far while the node runs, from microbatch batches or a loader's progress.
             */
            rows_so_far?: number | null;
            skipped_because_of?: string | null;
            /** Format: date-time */
            started_at?: string | null;
            status: string;
            /**
             * Format: int64
             * @description The rows a fully rebuilt table holds, read from the warehouse's metadata where that count is exact.
             */
            table_rows?: number | null;
        };
        /** @enum {string} */
        RunOrigin: "manual" | "schedule" | "sensor" | "rerun" | "backfill";
        /**
         * @description Terminal outcomes cannot be confused with a request to change an active status.
         * @enum {string}
         */
        RunOutcome: "success" | "warn" | "failed" | "cancelled" | "skipped";
        /** @description Captured output and error text of a run's invocations; both can quote warehouse data. */
        RunOutput: {
            /** @description Invocation failures and error diagnostics, oldest first. */
            errors: components["schemas"]["RunErrorText"][];
            /** @description The newest captured lines per invocation, oldest first. */
            lines: components["schemas"]["RunOutputLine"][];
        };
        /** @description What a run printed, per node, without its text; one node's error and warning lines; and what a size cap left out. */
        RunOutputIndex: {
            /**
             * Format: int64
             * @description Bytes the worker dropped because it could not send them fast enough.
             */
            dropped_bytes: number;
            /**
             * Format: int64
             * @description Lines the whole run printed, numbered in the order printed.
             */
            line_count: number;
            /** @description Lines the run's nodes logged with `ctx.log`, oldest first. */
            logs: components["schemas"]["NodeLogLine"][];
            /** @description The named node's lines that look like errors or warnings, by line; empty when no node is named. */
            marks: components["schemas"]["OutputMark"][];
            nodes: components["schemas"]["NodeOutputSummary"][];
            /**
             * Format: int64
             * @description Bytes and lines past `max_bytes_per_run` that were dropped between the run's first and last parts.
             */
            omitted_bytes: number;
            /** Format: int64 */
            omitted_lines: number;
        };
        RunOutputLine: {
            invocation_id: string;
            occurred_at: string;
            /** Format: int64 */
            sequence: number;
            stream: string;
            text: string;
        };
        /** @description A queued run's place in claim order within its location and target, and what holds it back. */
        RunQueueState: {
            /**
             * Format: int32
             * @description 1-based, in the order the scheduler claims: priority, then queue time.
             */
            position: number;
            /** @description Empty when nothing holds the run back and it starts on a free worker. */
            waits: components["schemas"]["QueueWait"][];
        };
        RunRecord: {
            /** Format: date-time */
            cancel_requested_at?: string | null;
            /** Format: date-time */
            ended_at?: string | null;
            git_dirty?: boolean | null;
            git_revision?: string | null;
            /**
             * Format: int32
             * @description Error-severity audit failures the run let through; a group sums its members'.
             */
            ignored_audit_failures?: number;
            /** @description The deployment image the run is pinned to, when its location publishes one. */
            image_id?: string | null;
            job_key?: string | null;
            kind: components["schemas"]["RunKind"];
            /** Format: int64 */
            last_event_storage_id: number;
            /** Format: date-time */
            launched_at?: string | null;
            /** Format: uuid */
            location_id: string;
            /**
             * Format: int32
             * @description Which attempt of its member job a group child is, once the group retried it; absent for a first attempt and for every other run.
             */
            member_attempt?: number | null;
            narrowing?: null | components["schemas"]["RunNarrowing"];
            node_counts_json: {
                [key: string]: number;
            };
            origin: components["schemas"]["RunOrigin"];
            /** Format: uuid */
            parent_run_id?: string | null;
            /** Format: int64 */
            priority: number;
            queue?: null | components["schemas"]["RunQueueState"];
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
            /**
             * Format: int64
             * @description The published version the run executes, read from its snapshot.
             */
            snapshot_generation?: number | null;
            /** Format: uuid */
            snapshot_id: string;
            /** @description The `sqb` version that published the snapshot. */
            sqb_version?: string | null;
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
            window?: null | components["schemas"]["RunWindow"];
        };
        /** @description The authorized selection and pinned execution inputs are immutable once requested. */
        RunRequest: {
            /**
             * Format: uuid
             * @description The first run of this run's rerun chain, read as `ctx.root_run_id`; unset for the first run.
             */
            chain_run_id?: string | null;
            command?: components["schemas"]["RunCommand"];
            image_id?: string | null;
            job_key?: string | null;
            /** @description The build-wide settings those jobs give where `settings` is silent; they decide its gates and write set. */
            job_settings?: {
                [key: string]: unknown;
            };
            /** @description The jobs an ad-hoc build's selection names, each pinned to its frozen nodes and settings. */
            jobs?: {
                [key: string]: components["schemas"]["PinnedJob"];
            };
            kind: components["schemas"]["RunKind"];
            /** Format: uuid */
            location_id: string;
            /**
             * Format: int32
             * @description Which attempt of its member job a group child is, from the second on; the first leaves it unset.
             */
            member_attempt?: number | null;
            narrowing?: null | components["schemas"]["RunNarrowing"];
            origin: components["schemas"]["RunOrigin"];
            /** Format: uuid */
            parent_run_id?: string | null;
            /** @description Each pool the run takes, its amount and the pool's limit frozen with it; the server's cap may lower the limit. */
            pools?: {
                [key: string]: components["schemas"]["PoolSlots"];
            };
            /** Format: int64 */
            priority: number;
            requested_by: string;
            /** Format: uuid */
            rerun_of_run_id?: string | null;
            retry?: null | components["schemas"]["RunRetry"];
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
        /** @description Which retry of the job's `[retry]` policy a run is, and when it may start. */
        RunRetry: {
            /** Format: int32 */
            attempt: number;
            /** Format: date-time */
            not_before: string;
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
        RunStatus: "queued" | "launched" | "running" | "unresponsive" | "success" | "warn" | "failed" | "cancelled" | "skipped";
        /** @description What started a run: a schedule or sensor by name, or a person or service account (`api`). */
        RunTrigger: {
            kind: components["schemas"]["RunTriggerKind"];
            /** @description The schedule's or sensor's name; null for manual and API runs. */
            name?: string | null;
            /** @description Who asked for a manual or API run, by display name. */
            person?: string | null;
        };
        /** @enum {string} */
        RunTriggerKind: "schedule" | "sensor" | "manual" | "api";
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
            actor_kind?: null | components["schemas"]["PrincipalKind"];
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
        /** @description The nodes `sqb build --select … --exclude …` selects in the current publication, or why it refuses the selectors. */
        SelectionEvaluation: {
            /**
             * Format: int64
             * @description How many of them are audits, keyed `audit:<name>[:…]`.
             */
            audits?: number;
            /**
             * Format: int64
             * @description Every selected node and audit, across all pages.
             */
            count: number;
            error?: null | components["schemas"]["SelectionProblem"];
            /** @description One page of the selected `kind:name` keys, sorted. */
            keys: string[];
            /**
             * Format: int32
             * @description The offset of the next page, when there is one.
             */
            next_offset?: number | null;
            /**
             * Format: uuid
             * @description The publication evaluated; `None` before the first one.
             */
            snapshot_id?: string | null;
        };
        /** @enum {string} */
        SelectionField: "select" | "exclude";
        /** @description One selector token in the request's `select` or `exclude` text. */
        SelectionLocation: {
            /** Format: int32 */
            end: number;
            field: components["schemas"]["SelectionField"];
            /**
             * Format: int32
             * @description Where the token starts and ends in its field, in UTF-16 code units, as JavaScript indexes strings.
             */
            start: number;
            token: string;
        };
        /** @description Why `sqb` refuses a selection, where, and what might have been meant. */
        SelectionProblem: {
            /** @description The selector error code, such as `S007`; `None` when the selection as a whole is refused. */
            code?: string | null;
            /** @description `sqb`'s own help line, such as "did you mean 'orders'?". */
            help?: string | null;
            location?: null | components["schemas"]["SelectionLocation"];
            message: string;
            refused?: null | components["schemas"]["SelectionRefusal"];
            /** @description Replacements for that whole token, closest first. */
            suggestions: string[];
        };
        /** @description One refused comma part: its exact text and position in the token, its selector type and value. */
        SelectionRefusal: {
            /**
             * Format: int32
             * @description Which comma part of the token it is, from 0.
             */
            index: number;
            /** @description The part as typed, such as `+tag:finanse`. */
            part: string;
            /** @description `tag`, `path`, `job`, `source`, `seed`, `kind`, `test`, `name`, `glob`, or a Python node type such as `asset`. */
            type: string;
            /** @description The value the part names, without its type prefix or `+` markers, such as `finanse`. */
            value: string;
        };
        /** @description The values each selector type takes in the current publication, with how many nodes each one selects. */
        SelectionVocabulary: {
            /** @description `audit:` names: the audits each name selects, one per target it is attached to. */
            audits?: components["schemas"]["VocabularyEntry"][];
            /** @description `job:` names: the nodes each job or job group selects. */
            jobs: components["schemas"]["VocabularyEntry"][];
            /** @description `kind:` values: the nodes of each kind. */
            kinds: components["schemas"]["VocabularyEntry"][];
            /** @description `path:` folders such as `models/staging`: the nodes below each, at any depth. */
            paths: components["schemas"]["VocabularyEntry"][];
            /** @description `seed:` names, one node each. */
            seeds: components["schemas"]["VocabularyEntry"][];
            /**
             * Format: uuid
             * @description The publication described; `None` before the first one.
             */
            snapshot_id?: string | null;
            /** @description `source:` names, one node each. */
            sources: components["schemas"]["VocabularyEntry"][];
            /** @description `tag:` values: the SQL resources and Python nodes with each tag. */
            tags: components["schemas"]["VocabularyEntry"][];
        };
        /** @description What wakes a sensor; a sensor without one polls every `minimum_interval_seconds`. */
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
            kind: "check_event";
            kinds: components["schemas"]["CheckEventKind"][];
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
        };
        /** @description A sensor: the function the location runs, and the condition that wakes it. */
        SensorDefinition: {
            code_hash: string;
            declared_at?: null | components["schemas"]["DeclaredAt"];
            default_status?: components["schemas"]["TriggerStatus"];
            description: string;
            /** @description The decorated function's name, such as `orders_failed`. */
            function: string;
            /** Format: int32 */
            minimum_interval_seconds: number;
            /** @description The targets the declaration is limited to; empty for every target. */
            targets?: string[];
            /**
             * Format: int32
             * @description How long one call of the function may take before its tick fails.
             */
            timeout_seconds: number;
            when?: null | components["schemas"]["SensorCondition"];
        };
        /** @description One event a sensor's function is called with; `id` is stable across redeliveries. */
        SensorEvent: {
            error?: string | null;
            failed_checks: string[];
            failed_nodes: string[];
            id: string;
            job: string;
            /** @enum {string} */
            kind: "run";
            /** Format: date-time */
            occurred_at: string;
            /** Format: uuid */
            run_id: string;
            status: components["schemas"]["RunOutcome"];
            target: string;
            url?: string | null;
        } | {
            event: components["schemas"]["NodeEventKind"];
            id: string;
            job?: string | null;
            /** @enum {string} */
            kind: "node";
            node: string;
            /** Format: date-time */
            occurred_at: string;
            /** Format: uuid */
            run_id: string;
            target: string;
            url?: string | null;
        } | {
            check: string;
            id: string;
            job?: string | null;
            /** @enum {string} */
            kind: "check";
            /** Format: date-time */
            occurred_at: string;
            /** Format: uuid */
            run_id: string;
            status: components["schemas"]["CheckEventKind"];
            target: string;
            url?: string | null;
        } | {
            data_version?: string | null;
            id: string;
            /** @enum {string} */
            kind: "source_new_data";
            /** Format: date-time */
            occurred_at: string;
            source: string;
            target: string;
        } | {
            /** Format: int32 */
            after_seconds: number;
            data_version?: string | null;
            id: string;
            /** @enum {string} */
            kind: "source_stale";
            /** Format: date-time */
            occurred_at: string;
            source: string;
            target: string;
        } | {
            details: Record<string, never>;
            event: components["schemas"]["PlatformEventKind"];
            id: string;
            /** @enum {string} */
            kind: "platform";
            /** Format: date-time */
            occurred_at: string;
        };
        /** @description A sensor whose latest ticks failed: since when, and the event it is stuck on. */
        SensorFailing: {
            /** @description The event the failed ticks were called with; absent for a polling sensor. */
            event_id?: string | null;
            event_summary?: string | null;
            failed_ticks: number;
            /** @description Why the newest failing tick failed, when the function gave no error, as for a location that didn't answer. */
            reason?: string | null;
            /**
             * Format: date-time
             * @description When the first of the failed ticks since the last good one started.
             */
            since: string;
        };
        /** @description What one call of a sensor's function returned; only the fenced scheduler applies it. */
        SensorFunctionOutcome: {
            cursor?: string | null;
            /** @enum {string} */
            status: "handled";
        } | {
            cursor?: string | null;
            reason: string;
            /** @enum {string} */
            status: "skipped";
        } | {
            cursor?: string | null;
            runs: components["schemas"]["RequestedRun"][];
            /** @enum {string} */
            status: "requested";
        } | {
            error: string;
            /** @enum {string} */
            status: "failed";
        } | {
            /** @enum {string} */
            status: "timed_out";
        };
        /** @description A published sensor and the events a trial chose for it, newest first. */
        SensorTrialEvents: {
            events: components["schemas"]["SensorEvent"][];
            function: string;
            sensor: string;
            /** Format: int32 */
            timeout_seconds: number;
            when?: null | components["schemas"]["SensorCondition"];
        };
        /** @description How long the server keeps each kind of record. */
        ServerRetention: {
            /** Format: int64 */
            audit_days: number;
            /** Format: int64 */
            heartbeat_days: number;
            /** Format: int64 */
            location_health_days: number;
            /** Format: int64 */
            platform_alert_days: number;
            /**
             * Format: int64
             * @description Days a finished run keeps its log of events, statements and output.
             */
            run_log_days: number;
            /**
             * Format: int64
             * @description Finished runs per location target that keep their logs.
             */
            run_logs_kept: number;
            /** Format: int64 */
            sensor_evaluation_days: number;
            /**
             * Format: int64
             * @description Publications kept per location target.
             */
            snapshots_kept: number;
            /**
             * Format: int64
             * @description Builds of each node, and results of each check, the store mirrors from the warehouse.
             */
            state_history_versions: number;
            /** Format: int64 */
            statistics_days: number;
        };
        /** @description The server's version, storage, sign-in and retention, for administrators. */
        ServerSettings: {
            installation: components["schemas"]["InstallationRecord"];
            login_mode: components["schemas"]["LoginMode"];
            retention: components["schemas"]["ServerRetention"];
            /** @description Whether this server runs the scheduler; another server may hold the lease. */
            scheduler: boolean;
            version: string;
        };
        ServiceAccountRole: {
            role: components["schemas"]["BuiltinRole"];
        };
        ServiceAccountState: {
            disabled: boolean;
        };
        /** @description A service account as administrators see it: its role, state and working keys. */
        ServiceAccountSummary: {
            /** Format: date-time */
            created_at: string;
            created_by?: string | null;
            description?: string | null;
            /**
             * Format: date-time
             * @description While set, every key of the account is refused.
             */
            disabled_at?: string | null;
            /** @description Keys that work, or will once the account is enabled: the current key first, then one still in its overlap. */
            keys: components["schemas"]["ServiceKeySummary"][];
            /**
             * Format: date-time
             * @description The newest last use of any of its keys, stopped ones included.
             */
            last_used_at?: string | null;
            name: string;
            /** Format: uuid */
            principal_id: string;
            role?: null | components["schemas"]["BuiltinRole"];
        };
        /**
         * @description How a service account key came to be: made by SQLBuild, or chosen with the CLI's `--key-from-env`.
         * @enum {string}
         */
        ServiceKeyOrigin: "issued" | "imported";
        /** @description A rotation: how many minutes the replaced key keeps working, at most a day; none or 0 stops it at once. */
        ServiceKeyRotation: {
            /** Format: int64 */
            overlap_minutes?: number | null;
        };
        /** @description One working key of a service account; never the key itself. */
        ServiceKeySummary: {
            /** Format: date-time */
            created_at: string;
            /** @description Who made it; none when the control-store owner did, with the CLI. */
            created_by?: string | null;
            /**
             * Format: date-time
             * @description When a key replaced by a rotation stops working; none for the current key.
             */
            expires_at?: string | null;
            /** Format: uuid */
            key_id: string;
            /**
             * Format: date-time
             * @description The last request this key authenticated that wasn't refused.
             */
            last_used_at?: string | null;
            origin: components["schemas"]["ServiceKeyOrigin"];
            /** @description The non-secret start of the key, e.g. `sqbk_Q7m2xT`. */
            prefix: string;
        };
        SessionIdentity: {
            /** Format: date-time */
            expires_at: string;
            /** Format: uuid */
            principal_id: string;
            /** Format: uuid */
            session_id: string;
        };
        /** @description A trigger the scheduler skips because its stored job or pinned snapshot can't be read, or a sensor whose check failed; it clears once its location publishes again or, for a sensor, a later check completes. */
        SkippedTrigger: {
            /** Format: uuid */
            location_id: string;
            location_name: string;
            name: string;
            reason: string;
            /**
             * Format: date-time
             * @description When it was first skipped for this snapshot.
             */
            skipped_at: string;
            target: string;
            /** Format: uuid */
            trigger_id: string;
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
         * @description Where the control store keeps its data.
         * @enum {string}
         */
        StorageKind: "sqlite" | "postgresql";
        /**
         * @description The tables Recall writes.
         * @enum {string}
         */
        Table: "invocations" | "output" | "events" | "diagnostics" | "artifacts" | "artifact_refs" | "heartbeats" | "delivery" | "meta" | "conflicts";
        /** @enum {string} */
        TickOutcome: "requested" | "covered" | "skipped";
        TickRecord: {
            /** Format: uuid */
            active_run_id?: string | null;
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
        } | {
            event: string;
            /** @enum {string} */
            kind: "skip_event";
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
            declared_at?: null | components["schemas"]["DeclaredAt"];
            default_status?: components["schemas"]["TriggerStatus"];
            description: string;
            job_key: string;
            schedule: components["schemas"]["Schedule"];
            /** Format: date-time */
            starts_at?: string | null;
            /** @description The targets the declaration is limited to; empty for every target. */
            targets?: string[];
        };
        TriggerRecord: {
            definition_json: components["schemas"]["TriggerDeclaration"];
            /** @description The job or group a schedule starts; `None` for a sensor, whose function chooses. */
            job_key?: string | null;
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
        /** @description One selector value and the number of nodes it selects. */
        VocabularyEntry: {
            /** Format: int64 */
            count: number;
            value: string;
        };
        /** @description One reason a member waits: `from_node` of `upstream_job` feeds its `to_node`, directly or through nodes no member selects. */
        WaitReason: {
            from_node: string;
            to_node: string;
            upstream_job: string;
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
        /** @description One input version an execution consumed, or the row marking that the execution started. */
        WarehouseConsumptionRecord: {
            execution_id?: string | null;
            /** Format: int64 */
            input_count?: number | null;
            input_kind?: string | null;
            input_name?: string | null;
            input_version?: string | null;
            node_name?: string | null;
            node_type?: string | null;
            phase?: string | null;
            /** Format: date-time */
            recorded_at: string;
            run_id?: string | null;
            state_database?: string | null;
            state_schema: string;
            target_database?: string | null;
            target_name?: string | null;
            target_schema?: string | null;
            version_hash?: string | null;
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
        /**
         * @description Catalog observations require full scans; only immutable history supports a watermark.
         * @enum {string}
         */
        WarehouseHistoryKind: "fingerprints" | "source_freshness" | "audit_results" | "janitor_events" | "migrations" | "old_name_views" | "microbatches" | "column_migrations" | "node_results" | "consumption" | "observations" | "resource_costs" | "catalog" | "statistics";
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
        /** @description A display-only value a build observed, such as a cursor `MAX`; planning never reads it. */
        WarehouseObservationRecord: {
            cursor_type?: string | null;
            kind: string;
            node: string;
            observation_id: string;
            /** Format: date-time */
            observed_at: string;
            run_id: string;
            state_database?: string | null;
            state_schema: string;
            value: string;
        };
        /** @description What went wrong reading one page, without the warehouse's message, which may name hosts or users. */
        WarehouseReadFailure: {
            /** @enum {string} */
            kind: "read_error";
        } | {
            /** @enum {string} */
            kind: "timed_out";
            /** Format: int64 */
            seconds: number;
        };
        /** @description One estimated query cost observation of a run, resource and warehouse; refreshes append. */
        WarehouseResourceCostRecord: {
            /** Format: int64 */
            attempt: number;
            attributed_seconds: string;
            /** Format: int64 */
            bytes_scanned: number;
            /** Format: int64 */
            cluster_number?: number | null;
            estimated_credits: string;
            estimated_usd?: string | null;
            limitations: string;
            observation_id: string;
            /** Format: date-time */
            observed_at: string;
            orchestration_run_id?: string | null;
            /** Format: int64 */
            query_count: number;
            resource_name: string;
            resource_type: string;
            /** Format: date-time */
            run_completed_at: string;
            run_id: string;
            /** Format: date-time */
            run_started_at: string;
            state_database?: string | null;
            state_schema: string;
            /** Format: int64 */
            statements_expected: number;
            /** Format: int64 */
            statements_observed: number;
            status: string;
            target_name?: string | null;
            /** @description The price per credit `estimated_usd` used; None on rows recorded before prices were. */
            usd_per_credit?: string | null;
            /** @description `config`, `rate_sheet` or `list_price` (an estimate); None on rows recorded before prices were. */
            usd_per_credit_source?: string | null;
            warehouse_name: string;
            warehouse_size: string;
            warehouse_type?: string | null;
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
        /** @description Both times come from the server clock, so the reader compares them without clock skew. */
        WarehouseSyncDue: {
            /** Format: date-time */
            now: string;
            /** Format: date-time */
            runs_ended_at?: string | null;
            /**
             * Format: int32
             * @description The newest versions per node or check a full read of state history needs to deliver.
             */
            state_history_versions: number;
        };
        /** @description A registered reader asks whether a run ended since its last synchronization began. */
        WarehouseSyncDueRead: {
            /** Format: int64 */
            generation: number;
            /** Format: uuid */
            session_id: string;
        };
        /**
         * @description The stage a warehouse sync failed at; only a failed read also reports its redacted error.
         * @enum {string}
         */
        WarehouseSyncFailure: "watermark" | "read" | "delivery" | "busy";
        /** @description How far one synchronization has got, so people see it running and when it completed. */
        WarehouseSyncProgress: {
            /** @description This report's page was the synchronization's last, so it has completed. */
            completed: boolean;
            /**
             * Format: int64
             * @description Milliseconds since the synchronization began.
             */
            elapsed_millis: number;
            /** @description Failed reads in a row stopped the sync until a run, a request, a publication or `next_sync_at`. */
            halted?: boolean;
            /**
             * Format: int64
             * @description Kinds the synchronization reads: 14 for each state schema.
             */
            kinds: number;
            /**
             * Format: int64
             * @description The reported kind's position in the synchronization, from 1.
             */
            position: number;
            /**
             * Format: int64
             * @description Rows the synchronization has delivered, this report's included.
             */
            rows: number;
        };
        WarehouseSyncResponse: {
            applied: boolean;
            /**
             * Format: int64
             * @description Records the server mirrors of this table for the target, counted after a read's last page.
             */
            mirrored_records?: number | null;
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
    readServerSettings: {
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
                    "application/json": components["schemas"]["ServerSettings"];
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
    listServiceAccounts: {
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
                    "application/json": components["schemas"]["ServiceAccountSummary"][];
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
    createServiceAccount: {
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
                "application/json": components["schemas"]["NewServiceAccount"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IssuedServiceKey"];
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
    setServiceAccountDisabled: {
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
                "application/json": components["schemas"]["ServiceAccountState"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ServiceAccountSummary"];
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
    rotateServiceAccountKey: {
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
                "application/json": components["schemas"]["ServiceKeyRotation"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IssuedServiceKey"];
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
    revokeServiceAccountKey: {
        parameters: {
            query?: never;
            header: {
                /** @description Session CSRF token */
                "x-sqb-csrf": string;
            };
            path: {
                principal_id: string;
                key_id: string;
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
                    "application/json": components["schemas"]["ServiceAccountSummary"];
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
    setServiceAccountRole: {
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
                "application/json": components["schemas"]["ServiceAccountRole"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ServiceAccountSummary"];
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
    readLoginMode: {
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
                    "application/json": components["schemas"]["LoginModeAnswer"];
                };
            };
            /** @description People sign in on this server */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
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
    createPasswordPerson: {
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
                "application/json": components["schemas"]["PasswordPersonRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonCreated"];
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
    listRunAcknowledgements: {
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
            /** @description Runs marked handled in the last week */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunAcknowledgement"][];
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
    acknowledgeRun: {
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
                "application/json": components["schemas"]["AcknowledgementRequest"];
            };
        };
        responses: {
            /** @description Whether the run's acknowledgement changed */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            /** @description The run is not in this location and target, or the event ID was reused */
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
    listAttentionAcknowledgements: {
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
            /** @description Conditions marked handled within two snoozes, lapsed ones included, and the snooze */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AttentionAcknowledgements"];
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
    acknowledgeAttention: {
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
                "application/json": components["schemas"]["AttentionAcknowledgementRequest"];
            };
        };
        responses: {
            /** @description Whether the condition's acknowledgement changed */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MutationResponse"];
                };
            };
            /** @description The key, shape or reason is invalid, or the event ID was reused */
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
            /** @description changed events: all, runs, ticks, snapshot:<location>, node_status:<target>. Refetch REST on all/reconnect; no durable cursor. node_status carries version, previous_version and the changed states (empty when none changed), or full: refetch node states when full is set or previous_version is not the version held. */
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
            header?: {
                /** @description An ETag from an earlier response; 304 while every check's newest result is unchanged */
                "If-None-Match"?: string | null;
            };
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
                    /** @description "<snapshot_id>:<digest of the body>", or "<digest>" before the first publication */
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CheckResult"][];
                };
            };
            /** @description Unchanged since the If-None-Match ETag */
            304: {
                headers: {
                    ETag?: string;
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
                "application/json": components["schemas"]["PersonWorkRequest"];
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
    readPersonCostSummary: {
        parameters: {
            query?: {
                /** @description Runs that started in the last this many days; defaults to 7, capped at 90 */
                days?: number;
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
            /** @description Estimated query cost by job, warehouse size and node, most expensive first */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CostSummaryRecord"];
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
    readCostSelection: {
        parameters: {
            query?: {
                /** @description Builds that started in the last this many whole UTC days, today included; defaults to 7, capped at 90 */
                days?: number;
                /** @description Space-separated selectors, as `sqb build -s` takes them; empty selects every node */
                select?: string;
                /** @description Space-separated selectors, as `--exclude` takes them */
                exclude?: string;
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
            /** @description The selection's cost job by node, against the period before, with daily series by job and top-level folder */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CostSelectionRecord"];
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
    readCostSeries: {
        parameters: {
            query: {
                /** @description As `readCostSelection` */
                days?: number;
                /** @description As `readCostSelection` */
                select?: string;
                /** @description As `readCostSelection` */
                exclude?: string;
                /** @description A JSON array of at most 200 `CostSeriesGroup`s */
                groups: string;
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
            /** @description Each group's credits per day, in the groups' order */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CostSeriesBatch"];
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
    readDiscovered: {
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
                    "application/json": null | components["schemas"]["DiscoveredReadback"];
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
            /** @description This target's state and the newest health observations and instance check-ins it was computed from */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonLocationHealth"];
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
    readPersonNodeStates: {
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
            /** @description Every current node's state at the state version node_status events continue from */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NodeStatesRecord"];
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
            header?: {
                /** @description An ETag from an earlier response; 304 when the snapshot is unchanged */
                "If-None-Match"?: string | null;
            };
            path: {
                location_id: string;
                target: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Every node of the current snapshot without its state, which node-states serves; empty before the first publication */
            200: {
                headers: {
                    /** @description "<snapshot_id>"; absent before the first publication */
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CatalogNodeRecord"][];
                };
            };
            /** @description Unchanged since the If-None-Match ETag */
            304: {
                headers: {
                    ETag?: string;
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
    listPersonNodeCosts: {
        parameters: {
            query?: {
                /** @description Builds that started in the last this many days; defaults to 7, capped at 90 */
                days?: number;
                /** @description Maximum builds; defaults to 100, capped at 1000 */
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
            /** @description The node's estimated query cost in each build, newest first */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NodeCostRecord"][];
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
    readPlanDiff: {
        parameters: {
            query: {
                /** @description The planned model whose diff to read */
                model: string;
            };
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
                    "application/json": components["schemas"]["PlanDiffRead"];
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
            /** @description No diff of that model, or no view.sql on every current node */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    readPlanRows: {
        parameters: {
            query: {
                /** @description The section to page */
                section: "planning_error" | "window_before_cursor_start" | "full_refresh" | "query_changed" | "function_changed" | "config_changed" | "schema_changed" | "first_run" | "renamed" | "routine_models" | "migrations" | "column_migrations" | "table_type_conversions" | "retention" | "loader_reloads" | "loader_loads" | "source_reloads" | "source_loads" | "changed_functions" | "functions" | "first_run_seeds" | "changed_seeds" | "unchanged_seeds" | "python_nodes" | "held_back" | "unchanged" | "warnings";
                /** @description Rows to skip; 0 by default */
                offset?: number;
                /** @description At most this many rows, capped at 1000 */
                limit?: number;
                /** @description Only rows whose name contains this, ignoring case */
                name?: string;
            };
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
                    "application/json": null | components["schemas"]["PlanRows"];
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
    readPlanSummary: {
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
                    "application/json": null | components["schemas"]["PlanSummaryRead"];
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
    readPersonProviders: {
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
            /** @description Declared providers without setting values */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ProvidersRead"];
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
    requestRepublish: {
        parameters: {
            query?: {
                /** @description Seconds to wait for the new snapshot or the publish error, at most 60; 0 or absent returns at once */
                wait?: number;
            };
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
                "application/json": components["schemas"]["PersonWorkRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RepublishStatus"];
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
    readRepublish: {
        parameters: {
            query?: {
                /** @description Seconds to wait while the republish is pending, at most 60 */
                wait?: number;
            };
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
                    "application/json": null | components["schemas"]["RepublishStatus"];
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
                /** @description Page size; defaults to 100, capped at 1000 */
                limit?: number;
                /** @description With before_run_id: continue after this run of the previous page */
                before_queued_at?: string;
                before_run_id?: string;
                /** @description List queued runs in claim order, each with its position and what it waits on */
                queued?: boolean;
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
            /** @description Runs newest first, or the queue in claim order */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunRecord"][];
                };
            };
            /** @description Only one of the before_* pair was given, or a cursor with queued */
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
    readPersonRunCost: {
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
            /** @description The latest estimated query cost observation of the run and of each resource and warehouse it used; empty when none was synced */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseResourceCostRecord"][];
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
    listPersonRunIgnoredAudits: {
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
            /** @description The error-severity audit failures the run and its current group members let through, run by run */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RunIgnoredAuditFailure"][];
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
    downloadPersonRunOutput: {
        parameters: {
            query: {
                node: string;
                stream?: components["schemas"]["OutputStream"];
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
            /** @description The node's whole stream */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/plain": string;
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
            /** @description The server keeps no run output */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    readPersonRunOutputIndex: {
        parameters: {
            query?: {
                node?: string;
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
                    "application/json": components["schemas"]["RunOutputIndex"];
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
    readPersonRunOutputLines: {
        parameters: {
            query?: {
                /** @description The node key, or `run`; leave it out to read the whole run's output. */
                node?: string;
                /** @description Only this stream's lines, keeping their numbers. */
                stream?: components["schemas"]["OutputStream"];
                /** @description Read forwards from this line, from 1. */
                from?: number;
                /** @description Read the rows that end just before this line. */
                before?: number;
                /** @description Read the last rows. */
                tail?: boolean;
                /** @description How many rows to read, at most 2,000. */
                limit?: number;
                /** @description Fold four or more lines that are the same but for their numbers into one row. */
                collapse?: boolean;
                /** @description A line to keep open, with three lines either side, when folding. */
                around?: number;
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
                    "application/json": components["schemas"]["OutputPage"];
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
            /** @description The server keeps no run output */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
            };
        };
    };
    searchPersonRunOutput: {
        parameters: {
            query: {
                node: string;
                stream?: components["schemas"]["OutputStream"];
                text: string;
                /** @description Look from this line, which a forward search includes, as a backward one does. */
                from?: number;
                direction?: "forward" | "backward";
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
                    "application/json": components["schemas"]["OutputSearch"];
                };
            };
            /** @description Empty search text */
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
            /** @description The server keeps no run output */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
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
    evaluateSelection: {
        parameters: {
            query?: {
                /** @description Space-separated selectors, as `sqb build -s` takes them; empty selects every node */
                select?: string;
                /** @description Space-separated selectors, as `--exclude` takes them */
                exclude?: string;
                /** @description Sorted keys to skip */
                offset?: number;
                /** @description Page size; defaults to 1,000, clamped to 1..10,000 */
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
            /** @description The selected keys, or why `sqb` refuses the selectors */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SelectionEvaluation"];
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
    readPastRuns: {
        parameters: {
            query?: {
                /** @description Comma-separated run IDs, each full or a unique prefix of at least 8 characters */
                ids?: string;
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
            /** @description What each run selected, didn't finish and skipped, or why one can't be read */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PastRunsRead"];
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
    readSelectionVocabulary: {
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
                    "application/json": components["schemas"]["SelectionVocabulary"];
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
    readPersonSensorEvents: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                location_id: string;
                target: string;
                name: string;
                limit: number | null;
                latest: boolean | null;
                event: string | null;
                run: string | null;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The published sensor and the chosen events, newest first; reading them changes nothing */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SensorTrialEvents"];
                };
            };
            /** @description More than one of latest, event and run */
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
            /** @description No such sensor, event or matching run, and why */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
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
    readPersonSensorState: {
        parameters: {
            query?: {
                /** @description Maximum recent checks and run requests; defaults to 100, capped at 1000 */
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
            /** @description The cursor, recent checks newest first and requested runs */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonSensorState"];
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
            /** @description Sensor not found for a person who may view the whole target */
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
    requestWarehouseSync: {
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
                "application/json": components["schemas"]["PersonWorkRequest"];
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
    listPersonPlatformEvents: {
        parameters: {
            query?: {
                /** @description Continue after this storage ID of the previous page */
                before?: number;
                /** @description Events to scan; defaults to 200, clamped to 1..500 */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One scan of stored platform events newest first, without heartbeats: server-wide ones, and those of targets the person lists */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonPlatformEventPage"];
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
            query?: {
                /** @description Only this target's observations */
                target?: string;
            };
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
    report_changed_build: {
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
                "application/json": components["schemas"]["ChangedBuildReport"];
            };
        };
        responses: {
            /** @description Stored; it replaces the target's previous report */
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
            /** @description Reader generation fenced */
            409: {
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
            /** @description changed events: all, runs, run:<id>, ticks, snapshot:<location>, node_status:<target>. Refetch REST on all/reconnect; no durable cursor. node_status carries version, previous_version and the changed states (empty when none changed), or full: refetch node states when full is set or previous_version is not the version held. */
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
    preview_copy: {
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
                "application/json": components["schemas"]["PreviewCopyRead"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PreviewCopy"];
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
    sync_due: {
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
                "application/json": components["schemas"]["WarehouseSyncDueRead"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WarehouseSyncDue"];
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
    output_batch: {
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
                "application/json": components["schemas"]["OutputBatch"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OutputBatchReceipt"];
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
            /** @description The server keeps no run output */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ApiErrorBody"];
                };
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
