import { goto } from '$app/navigation';
import type { RerunMode } from '$lib/api';
import { app } from '$lib/state/app.svelte';
import { createResource } from '$lib/state/resource.svelte';
import { isActiveRun } from '$lib/status';
import type { BottomTab } from './types';
import { bannersFor, primaryChoice, progressOf, rerunChoices } from './utils';

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** Run detail state: data, the node selection that filters the bottom pane, and actions. */
export function createRunDetailState(runId: string) {
	const topics = [`run:${runId}`, 'runs'];
	const detailRes = createResource((client, scope) => client.getRunDetail(scope, runId), topics);
	const eventsRes = createResource((client, scope) => client.listRunEvents(scope, runId, { limit: 5000 }), topics);
	const outputRes = createResource((client, scope) => client.listRunOutput(scope, runId), topics);
	const statementsRes = createResource((client, scope) => client.listRunStatements(scope, runId), topics);

	const detail = $derived(detailRes.data ?? null);
	const run = $derived(detail?.run ?? null);
	const nodes = $derived(detail?.nodes ?? []);
	const active = $derived(!!run && isActiveRun(run.status));
	const progress = $derived(progressOf(nodes));
	const banners = $derived(detail ? bannersFor(detail, app.now) : []);
	const statements = $derived(statementsRes.data ?? []);
	// Progress reported by a run with no signal is stale, so the live card only shows while it's running.
	const runningStatements = $derived(run?.status === 'running' ? statements.filter((s) => s.status === 'running') : []);
	let showDetails = $state(false);

	// ---- node selection: the one filter model for timeline, panel and bottom tabs ----
	let selected = $state<string[]>([]);
	let tab = $state<BottomTab>('statements');

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
	const choices = $derived(detail ? rerunChoices(detail, selected, (c) => app.can(c)) : []);
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

	async function rerun(mode: RerunMode) {
		const scope = app.scope;
		if (!scope || !run) return;
		busy = mode;
		actionError = null;
		try {
			const next = await app.client.rerunRun(scope, run.id, mode, mode === 'selected' ? selected : undefined);
			await goto(`/runs/${next.id}`);
		} catch (e) {
			actionError = message(e);
		} finally {
			busy = null;
		}
	}

	return {
		detailRes,
		eventsRes,
		outputRes,
		statementsRes,
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
