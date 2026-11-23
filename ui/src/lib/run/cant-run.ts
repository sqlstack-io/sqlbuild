// What a plan can't run because a provider is missing secrets, and the words the plan page shows for it. Pure.

import type { PlanCantRun, Run, RunDetail, RunNode } from '$lib/api';
import { plural } from '$lib/format/plural';
import { buildCommand, parseCommand, selectorOf } from './command';

/** A node key's name: `raw_payments` for `source:raw_payments`. */
export const keyName = (key: string) => key.slice(key.indexOf(':') + 1);

/** Every node the plan can't run, the providers' users before their readers, each once. */
export function cantRunKeys(entries: PlanCantRun[]): string[] {
	const keys = new Set<string>();
	for (const e of entries) for (const key of e.nodes) keys.add(key);
	for (const e of entries) for (const key of e.readers) keys.add(key);
	return [...keys];
}

/** The unset variables, each once. */
export function missingVariables(entries: PlanCantRun[]): string[] {
	return [...new Set(entries.flatMap((e) => e.variables))];
}

/** "a", "a and b", "a, b and c". */
const andList = (parts: string[]) => (parts.length < 2 ? parts.join('') : `${parts.slice(0, -1).join(', ')} and ${parts.at(-1)}`);

/** "sink run_log needs LOG_TOKEN; every run uses sinks", once per sink a missing secret stops. */
export function sinkLines(entries: PlanCantRun[]): string[] {
	return entries.flatMap((e) => (e.sinks ?? []).map((sink) => `sink ${sink} needs ${andList(e.variables)}; every run uses sinks`));
}

/** Whether a sink's missing secret stops the whole plan, so leaving nodes out can't help. */
export const sinkBlocks = (entries: PlanCantRun[]) => entries.some((e) => (e.sinks ?? []).length > 0);

/** The plan's Will run count without the nodes it can't run; `cant_run` lists only nodes the plan runs, never ones held back or skipped. */
export const willRunCount = (run: number, entries: PlanCantRun[]) => (sinkBlocks(entries) ? 0 : Math.max(0, run - cantRunKeys(entries).length));

/** Why Run is disabled: "Run is disabled: 1 missing secret. Fix it and plan again, or leave those 3 nodes out." */
export function cantRunReason(entries: PlanCantRun[]): string {
	const sinks = sinkLines(entries);
	if (sinks.length) return `Run is disabled: ${sinks.join('; ')}. Fix ${sinks.length === 1 ? 'it' : 'them'} and plan again.`;
	const missing = missingVariables(entries).length;
	const nodes = cantRunKeys(entries).length;
	return `Run is disabled: ${plural(missing, 'missing secret')}. Fix ${missing === 1 ? 'it' : 'them'} and plan again, or leave ${nodes === 1 ? 'that node' : `those ${nodes} nodes`} out.`;
}

/** The command with `keys` added to its `--exclude`, each once. */
export function withoutNodes(text: string, keys: string[]): string {
	const parts = parseCommand(text);
	const exclude = [...parts.exclude];
	for (const selector of keys.map(selectorOf)) if (!exclude.includes(selector)) exclude.push(selector);
	return buildCommand({ ...parts, exclude });
}

/** The heading's variables: "PAYMENTS_API_TOKEN", or "A and B". */
export const variablesText = (entry: PlanCantRun) => andList(entry.variables);

/** What the provider stops: "raw_payments and the 2 nodes that read it (stg_payments, fct_payments)". */
export function needsText(entry: PlanCantRun): string {
	const users = andList(entry.nodes.map(keyName));
	if (!entry.readers.length) return users;
	const it = entry.nodes.length === 1 ? 'it' : 'them';
	return `${users} and the ${plural(entry.readers.length, 'node')} that read ${it} (${entry.readers.map(keyName).join(', ')})`;
}

/**
 * How to fix it at the location, then `then`: "plan again" on a plan, "the next run works" on a run. A location reads
 * its env file again when it restarts, or on `r` under `sqb dev`; only a restart sees a changed environment.
 */
export function fixText(entry: PlanCantRun, then: string): string {
	const environment = 'in the environment the location (or sqb dev) was started with, then restart it';
	const file = entry.env_file ? `in ${entry.env_file}, then restart the location (in sqb dev, press r), or ` : '';
	return `set ${variablesText(entry)} ${file}${environment}; after that, ${then}`;
}

/** One table row: the node, whether it uses the provider or only reads a node that does, and why it can't run. */
export type CantRunRow = { key: string; uses: boolean; why: string };

/**
 * A row per node the plan can't run: users first, naming the provider and its unset variables, then readers,
 * naming what they read that can't run, from `upstreamOf` when it knows, else the provider's users.
 */
export function cantRunRows(entries: PlanCantRun[], upstreamOf: (key: string) => string[] = () => []): CantRunRow[] {
	const rows = new Map<string, CantRunRow>();
	for (const e of entries) {
		for (const key of e.nodes) if (!rows.has(key)) rows.set(key, { key, uses: true, why: `provider ${e.provider}: ${andList(e.variables)} not set` });
	}
	const blocked = new Set(cantRunKeys(entries));
	for (const e of entries) {
		for (const key of e.readers) {
			if (rows.has(key)) continue;
			const read = upstreamOf(key).filter((up) => blocked.has(up));
			rows.set(key, { key, uses: false, why: `reads ${andList((read.length ? read : e.nodes).map(keyName))}` });
		}
	}
	return [...rows.values()];
}

/**
 * A run node's `D012` message, as a worker writes it and the server lists it: `provider X: A, B not set (missing from
 * the location's environment and from .env)`. Error redaction can take a variable of a provider named like a secret
 * (`orders_api_key: [REDACTED]`), so the variables come from the run's status reason where it has them.
 */
const MISSING_SECRET_MESSAGE = /^provider (\S+): (.+) not set \(missing from (.+)\)$/;

/**
 * The status reason of a run that stopped on missing secrets: `Couldn't run: provider X needs A, B; sink S needs C;
 * every run uses sinks (missing from the location's environment and from .env); nothing ran`, perhaps with a note after it.
 */
const MISSING_SECRET_REASON = /^Couldn't run: (.+) \(missing from (.+)\); nothing ran/;

/** The env file in where a secret was looked for: `.env` in `the location's environment and from .env`. */
const envFileIn = (places: string) => /environment and from (.+)$/.exec(places)?.[1] ?? null;

type Refusal = { envFile: string | null; providers: Map<string, string[]>; sinks: { name: string; variables: string[] }[] };

function refusalOf(reason: string | null): Refusal | null {
	const match = MISSING_SECRET_REASON.exec(reason ?? '');
	if (!match) return null;
	const refusal: Refusal = { envFile: envFileIn(match[2]), providers: new Map(), sinks: [] };
	for (const part of match[1].split('; ')) {
		const provider = /^provider (\S+) needs (.+)$/.exec(part);
		if (provider) refusal.providers.set(provider[1], provider[2].split(', '));
		const sink = /^sink (.+) needs (.+)$/.exec(part);
		if (sink) refusal.sinks.push({ name: sink[1], variables: sink[2].split(', ') });
	}
	return refusal.providers.size || refusal.sinks.length ? refusal : null;
}

/** The run's nodes, as much of them as this needs. */
type RunNodeFacts = Pick<RunNode, 'runId' | 'nodeKey' | 'status' | 'startedAt' | 'errorCode' | 'errorMessage'> & { skippedBecauseOf: { nodeKey: string } | null };

/**
 * What runs couldn't run because providers were missing secrets: from each run's status reason, the providers, their
 * variables and any sinks, which fail no node; from its failed nodes' messages, who uses which provider; and the
 * nodes skipped because of those, directly or through another skipped node. Only `runs` the nodes belong to count,
 * so a member's superseded attempt adds nothing. A sink whose provider no node uses has no provider name to show.
 */
export function runCantRun(nodes: RunNodeFacts[], runs: Pick<Run, 'id' | 'statusReason'>[]): PlanCantRun[] {
	const shown = new Set(nodes.map((node) => node.runId));
	const entries = new Map<string, PlanCantRun>();
	const entryOf = (id: string, provider: string, variables: string[], envFile: string | null) => {
		const entry = entries.get(id) ?? { reason: 'missing_secret', provider, path: '', variables: [], nodes: [], readers: [], sinks: [], env_file: envFile };
		entry.variables = [...new Set([...entry.variables, ...variables])];
		entries.set(id, entry);
		return entry;
	};
	const refusals = new Map<string, Refusal>();
	for (const run of runs) {
		const refusal = shown.has(run.id) ? refusalOf(run.statusReason) : null;
		if (!refusal) continue;
		refusals.set(run.id, refusal);
		for (const [provider, variables] of refusal.providers) entryOf(provider, provider, variables, refusal.envFile);
		for (const { name, variables } of refusal.sinks) {
			const wanted = variables.join(', ');
			const provider = [...refusal.providers].find(([, own]) => own.join(', ') === wanted)?.[0] ?? '';
			const entry = entryOf(provider || `sink:${wanted}`, provider, variables, refusal.envFile);
			if (!entry.sinks?.includes(name)) entry.sinks = [...(entry.sinks ?? []), name];
		}
	}
	const users = new Map<PlanCantRun, Set<string>>();
	const owner = new Map<string, PlanCantRun>();
	for (const node of nodes) {
		if (node.status !== 'failed' || node.errorCode !== 'D012') continue;
		const refusal = refusals.get(node.runId);
		const said = node.errorMessage ? MISSING_SECRET_MESSAGE.exec(node.errorMessage) : null;
		// Without a message, as once history is pruned, only a run missing one provider's secrets says whose it was.
		const provider = said?.[1] ?? (!node.startedAt && refusal?.providers.size === 1 ? [...refusal.providers.keys()][0] : null);
		if (!provider) continue;
		const variables = refusal?.providers.get(provider) ?? said?.[2].split(', ') ?? [];
		const entry = entryOf(provider, provider, variables, refusal?.envFile ?? (said ? envFileIn(said[3]) : null));
		const own = users.get(entry) ?? new Set<string>();
		if (!own.has(node.nodeKey)) entry.nodes.push(node.nodeKey);
		users.set(entry, own.add(node.nodeKey));
		owner.set(node.nodeKey, entry);
	}
	let added = true;
	while (added) {
		added = false;
		for (const node of nodes) {
			const cause = node.skippedBecauseOf?.nodeKey;
			const entry = cause ? owner.get(cause) : undefined;
			if (node.status !== 'skipped' || !entry || owner.has(node.nodeKey)) continue;
			entry.readers.push(node.nodeKey);
			owner.set(node.nodeKey, entry);
			added = true;
		}
	}
	return [...entries.values()];
}

/** The run page's Couldn't run blocks: its nodes, with the status reasons of the run and, for a group, its job runs. */
export const runDetailCantRun = (detail: Pick<RunDetail, 'run' | 'children' | 'nodes'>) => runCantRun(detail.nodes, [detail.run, ...detail.children]);
