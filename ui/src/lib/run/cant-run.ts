// What a plan can't run because a provider is missing secrets, and the words the plan page shows for it. Pure.

import type { PlanCantRun } from '$lib/api';
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

/** The plan's Will run count without the nodes it can't run. */
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

/** How to fix it at the location, then `then`: "plan again" on a plan, "the next run works" on a run. */
export function fixText(entry: PlanCantRun, then: string): string {
	const where = entry.env_file ? `the location's environment (or its ${entry.env_file})` : "the location's environment";
	return `set ${variablesText(entry)} in ${where}, restart the location, then ${then}`;
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

/** A run node's `D012` message, as a worker writes it: `provider X: A, B not set (missing from the location's environment and from .env)`. */
const MISSING_SECRET_MESSAGE = /^provider (\S+): (.+) not set \(missing from (.+)\)$/;

/** The run's nodes, as much of them as this needs. */
type RunNodeFacts = { nodeKey: string; status: string; errorCode: string | null; errorMessage: string | null; skippedBecauseOf: { nodeKey: string } | null };

/**
 * What a run couldn't run because a provider was missing secrets, read from its nodes: each provider's failed
 * nodes, and the nodes skipped because of them, directly or through another skipped node.
 */
export function runCantRun(nodes: RunNodeFacts[]): PlanCantRun[] {
	const byProvider = new Map<string, PlanCantRun>();
	const owner = new Map<string, PlanCantRun>();
	for (const node of nodes) {
		if (node.status !== 'failed' || node.errorCode !== 'D012' || !node.errorMessage) continue;
		const match = MISSING_SECRET_MESSAGE.exec(node.errorMessage);
		if (!match) continue;
		const [, provider, variables, places] = match;
		const envFile = /and from (.+)$/.exec(places)?.[1] ?? null;
		const entry = byProvider.get(provider) ?? { reason: 'missing_secret', provider, path: '', variables: variables.split(', '), nodes: [], readers: [], sinks: [], env_file: envFile };
		entry.nodes.push(node.nodeKey);
		byProvider.set(provider, entry);
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
	return [...byProvider.values()];
}
