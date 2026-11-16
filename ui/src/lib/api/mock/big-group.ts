// A large job group, for layout tests and for looking at how a big group draws: members in stages,
// each waiting for two members of the stage before it, and a few members on their own. The server
// lists every wait, the implied ones too, so each member's waits are its whole upstream. Pure.

export const BIG_GROUP = 'warehouse_nightly';

const STAGES = ['load', 'stage', 'clean', 'join', 'enrich', 'aggregate', 'publish'];
const SIZES = [4, 5, 5, 5, 5, 5, 5];
const ALONE = 6;

export type BigGroupShape = {
	/** Members in declaration order. */
	members: string[];
	/** What each member waits for directly. */
	direct: Map<string, string[]>;
	/** What the server lists: every member upstream of each one. */
	waits: Map<string, string[]>;
	/** Members with no waits either way. */
	alone: string[];
};

/**
 * The mock catalog's node each stage's members build: a lineage chain, so most waits have lineage
 * behind them. activity_report doesn't read daily_activity_rollup, so publish's waits on aggregate
 * have none and draw dashed, as an ordering-only wait does.
 */
const NODES: Record<string, string> = {
	load: 'raw__orders',
	stage: 'stg_orders',
	clean: 'fact_orders',
	join: 'hourly_order_activity',
	enrich: 'hourly_activity_ctx',
	aggregate: 'daily_activity_rollup',
	publish: 'activity_report',
	housekeeping: 'country_codes'
};

/**
 * Where a member is declared: a folder per stage under `jobs/warehouse/`, and one export job alone in
 * its own folder, which the group page draws as the job's card rather than a folder.
 */
export const bigGroupFile = (member: string): string => `jobs/warehouse/${member === 'publish_e' ? 'exports' : member.split('_')[0]}/${member}.py`;

/** The node a member builds, as a selector. */
export const bigGroupSelect = (member: string): string => NODES[member.split('_')[0]];

export function bigGroupShape(): BigGroupShape {
	const stages = STAGES.map((name, s) => Array.from({ length: SIZES[s] }, (_, i) => `${name}_${String.fromCharCode(97 + i)}`));
	const alone = Array.from({ length: ALONE }, (_, i) => `housekeeping_${i + 1}`);
	const direct = new Map<string, string[]>();
	stages.forEach((stage, s) =>
		stage.forEach((m, i) => {
			const prev = stages[s - 1] ?? [];
			direct.set(m, prev.length ? [...new Set([prev[i % prev.length], prev[(i + 1) % prev.length]])] : []);
		})
	);
	for (const m of alone) direct.set(m, []);
	const upstreamOf = (m: string): Set<string> => new Set((direct.get(m) ?? []).flatMap((u) => [u, ...upstreamOf(u)]));
	const members = [...alone.slice(0, 3), ...stages.flat(), ...alone.slice(3)];
	return {
		members,
		direct,
		waits: new Map(members.map((m) => [m, [...upstreamOf(m)]])),
		alone
	};
}
