import { to } from '$lib/paths';
import { goto } from '$app/navigation';
import type { RerunMode } from '$lib/api';
import { app } from '$lib/state/app.svelte';
import { rerunHref } from '$lib/run/command';
import { createResource } from '$lib/state/resource.svelte';
import { isActiveRun } from '$lib/status';
import type { BottomTab } from './types';
import { bannersFor, primaryChoice, progressOf, rerunChoices, runCostOf } from './utils';

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** Run detail state: data, the node selection that filters the bottom pane, and actions. */
export function createRunDetailState(runId: string) {
	const topics = [`run:${runId}`, 'runs'];
	const detailRes = createResource((client, scope) => client.getRunDetail(scope, runId), topics);
	// A group run's own reads are nearly empty, so its log and statements gather its job runs' too.
	// A string, so the reads below reload only when the set of runs changes, not on every detail reload.
	const runKey = $derived([runId, ...(detailRes.data?.children ?? []).map((c) => c.id)].join(','));
	const eachRun = async <T>(read: (id: string) => Promise<T[]>): Promise<T[]> => (await Promise.all(runKey.split(',').map(read))).flat();
	const eventsRes = createResource(
		(client, scope) => (client.supports('listRunEvents') ? eachRun(async (id) => (await client.listRunEvents(scope, id, { limit: 5000 })).events).then((events) => events.sort((a, b) => a.occurredAt.localeCompare(b.occurredAt))) : Promise.resolve([])),
		topics
	);
	const outputRes = createResource(
		(client, scope) =>
			client.supports('listRunOutput') && app.can('view.evidence')
				? eachRun(async (id) => (await client.listRunOutput(scope, id)).map((line) => ({ ...line, runId: id }))).then((lines) => lines.sort((a, b) => a.at.localeCompare(b.at)))
				: Promise.resolve([]),
		topics
	);
	const statementsRes = createResource((client, scope) => (client.supports('listRunStatements') ? eachRun((id) => client.listRunStatements(scope, id)) : Promise.resolve([])), topics);
	const costRes = createResource((client, scope) => (client.supports('listRunCost') ? eachRun((id) => client.listRunCost(scope, id)) : Promise.resolve([])), topics);

	const detail = $derived(detailRes.data ?? null);
	const run = $derived(detail?.run ?? null);
	const nodes = $derived(detail?.nodes ?? []);
	const active = $derived(!!run && isActiveRun(run.status));
	const progress = $derived(progressOf(nodes));
	const cause = $derived(outputRes.data?.find((l) => l.error)?.text ?? null);
	const banners = $derived(detail ? bannersFor(detail, app.now, cause) : []);
	const statements = $derived(statementsRes.data ?? []);
	const cost = $derived(costRes.data ? runCostOf(costRes.data) : null);
	// Progress reported by a run with no signal is stale, so the live card only shows while it's running.
	const runningStatements = $derived(run?.status === 'running' ? statements.filter((s) => s.status === 'running') : []);
	let showDetails = $state(false);

	// ---- node selection: the one filter model for timeline, panel and bottom tabs ----
	let selected = $state<string[]>([]);
	let tab = $state<BottomTab>('log');

	/** Click selects one node (or clears it); with `add`, toggles it in a multi-selection. */
	function selectNode(key: string, add = false) {
		if (add) selected = selected.includes(key) ? selected.filter((k) => k !== key) : [...selected, key];
		else selected = selected.length === 1 && selected[0] === key ? [] : [key];
	}

	function clearSelection() {
		selected = [];
	}

	function focusNode(key: string, nextTab: BottomTab) {
		selected = [key];
		tab = nextTab;
	}

	// ---- actions ----
	const choices = $derived(detail ? rerunChoices(detail, app.client.supports('rerunSelectedNodes') ? selected : [], (c) => app.can(c)) : []);
	const primary = $derived(detail && choices.length ? primaryChoice(detail, choices) : null);
	let busy = $state<'cancel' | RerunMode | null>(null);
	let actionError = $state<string | null>(null);
	let cancelArmed = $state(false);

	async function cancel() {
		const scope = app.scope;
		if (!scope || !run) return;
		if (!cancelArmed) {
			cancelArmed = true;
			return;
		}
		cancelArmed = false;
		busy = 'cancel';
		actionError = null;
		try {
			await app.client.cancelRun(scope, run.id);
			detailRes.refresh();
		} catch (e) {
			actionError = message(e);
		} finally {
			busy = null;
		}
	}

	/** Reruns are reviewed first, like every other run. */
	function rerun(mode: RerunMode) {
		if (run) void goto(to(rerunHref([run.id], mode, mode === 'selected' ? selected : [])));
	}

	return {
		detailRes,
		eventsRes,
		outputRes,
		statementsRes,
		costRes,
		get cost() {
			return cost;
		},
		get detail() {
			return detail;
		},
		get run() {
			return run;
		},
		get nodes() {
			return nodes;
		},
		get active() {
			return active;
		},
		get progress() {
			return progress;
		},
		get banners() {
			return banners;
		},
		get statements() {
			return statements;
		},
		get runningStatements() {
			return runningStatements;
		},
		get selected() {
			return selected;
		},
		get showDetails() {
			return showDetails;
		},
		toggleDetails() {
			showDetails = !showDetails;
		},
		get tab() {
			return tab;
		},
		set tab(value: BottomTab) {
			tab = value;
		},
		selectNode,
		clearSelection,
		focusNode,
		get choices() {
			return choices;
		},
		get primary() {
			return primary;
		},
		get busy() {
			return busy;
		},
		get actionError() {
			return actionError;
		},
		get cancelArmed() {
			return cancelArmed;
		},
		disarmCancel() {
			cancelArmed = false;
		},
		cancel,
		rerun
	};
}

export type RunDetailState = ReturnType<typeof createRunDetailState>;
