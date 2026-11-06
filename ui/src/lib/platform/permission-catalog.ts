// Plain-language names and risk tags for every action the server checks.
// Shared by the admin pages, the access checker, the audit log and the
// "Requires …" hints on gated controls.

import { CAPABILITIES, type AuditEntry, type Capability, type PermissionRisk } from '$lib/api';

export type PermissionInfo = {
	/** Short plain name, e.g. "Run jobs". */
	label: string;
	/** One sentence on what it allows. */
	description: string;
	risk: PermissionRisk;
};

export const PERMISSIONS: Record<Capability, PermissionInfo> = {
	view: { label: 'See jobs, runs and results', description: 'Browse lineage, the catalog, runs, checks and triggers.', risk: 'read' },
	'view.sql': { label: 'See SQL', description: 'Read model SQL and the statements runs sent to the warehouse.', risk: 'read' },
	'view.evidence': { label: 'See sample rows', description: 'Open the sample and failing rows stored with check results. Every read is audited.', risk: 'read' },
	'job.run': { label: 'Run jobs', description: 'Start a declared job or group.', risk: 'operate' },
	'run.cancel': { label: 'Cancel runs', description: 'Stop a queued or running run.', risk: 'operate' },
	'run.rerun': { label: 'Rerun runs', description: 'Run a finished run again, in full or from where it failed.', risk: 'operate' },
	'run.rewrite_history': {
		label: 'Full refresh or reprocess past data',
		description: 'Rebuild tables from scratch or reprocess earlier periods, replacing data already built.',
		risk: 'changes_data'
	},
	'trigger.toggle': { label: 'Pause and resume schedules and sensors', description: 'Stop or restart automatic runs.', risk: 'operate' },
	'sensor.reset_cursor': { label: 'Reset where a sensor is up to', description: 'Move a sensor’s position so it looks at events again.', risk: 'operate' },
	'run.adhoc': { label: 'Build any selection of nodes', description: 'Start an ad-hoc run of nodes outside a declared job, writing to the warehouse.', risk: 'changes_data' },
	'connection.test': { label: 'Test warehouse connections', description: 'Run the connection check from a location.', risk: 'operate' },
	'plan.preview': { label: 'Preview build plans', description: 'See what a run would build without running it.', risk: 'read' },
	'location.manage': { label: 'Manage locations and their tokens', description: 'Register locations and create, rotate or revoke their tokens.', risk: 'administer' },
	'user.manage': { label: 'Manage people and service accounts', description: 'Disable or enable people and create service accounts.', risk: 'administer' },
	'role.assign': { label: 'Give roles and approve role changes', description: 'Assign and remove roles, and approve or reject roles that gain actions.', risk: 'administer' },
	'token.manage': { label: 'Manage service-account tokens', description: 'Create, rotate and revoke API tokens.', risk: 'administer' },
	'audit.view': { label: 'See the audit log', description: 'Read who did what, including denied requests.', risk: 'administer' }
};

export const RISK_LABEL: Record<PermissionRisk, string> = {
	read: 'read',
	operate: 'operate',
	changes_data: 'changes data',
	administer: 'administer'
};

export const RISK_ORDER: readonly PermissionRisk[] = ['read', 'operate', 'changes_data', 'administer'];

export const RISK_TONE: Record<PermissionRisk, string> = {
	read: 'var(--sb-text-faint)',
	operate: 'var(--sb-running)',
	changes_data: 'var(--sb-warning)',
	administer: 'var(--sb-error)'
};

/** Actions that change warehouse data or administer access. */
export function isRisky(action: Capability): boolean {
	const risk = PERMISSIONS[action].risk;
	return risk === 'changes_data' || risk === 'administer';
}

export function permissionLabel(action: Capability): string {
	return PERMISSIONS[action].label;
}

/** Every action, grouped by risk from least to most. */
export const PERMISSIONS_BY_RISK: { risk: PermissionRisk; actions: Capability[] }[] = RISK_ORDER.map((risk) => ({
	risk,
	actions: CAPABILITIES.filter((c) => PERMISSIONS[c].risk === risk)
}));

/** e.g. `Requires "Give roles and approve role changes" (role.assign)`. */
export function requiresText(action: Capability): string {
	return `Requires “${PERMISSIONS[action].label}” (${action})`;
}

const AUDIT_ONLY: Record<Exclude<AuditEntry['action'], Capability>, string> = {
	'role.approve': 'Approved a role change',
	'role.reject': 'Rejected a role change',
	'session.login': 'Signed in'
};

/** Plain name of an audited action, including the audit-only ones. */
export function auditActionLabel(action: AuditEntry['action']): string {
	return action in PERMISSIONS ? PERMISSIONS[action as Capability].label : AUDIT_ONLY[action as keyof typeof AUDIT_ONLY];
}

export const AUDIT_ACTIONS: AuditEntry['action'][] = [...CAPABILITIES, 'role.approve', 'role.reject', 'session.login'];
