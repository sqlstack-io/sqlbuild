import { to } from '$lib/paths';
import { goto } from '$app/navigation';
import type { RerunMode } from '$lib/api';
import { app } from '$lib/state/app.svelte';
import { rerunHref } from '$lib/run/command';
import { createResource } from '$lib/state/resource.svelte';
import { createSelection } from '$lib/state/selection.svelte';
import { nodeLabel } from '$lib/operate/labels';
import { planHref } from '$lib/run/command';
import { quickPickText, summaryOf, withNameToggled, type QuickPick } from './utils/proto';
import { isActiveRun } from '$lib/status';
import type { BottomTab, NodeGroupKey } from './types';
import { bannersFor, groupNodes, primaryChoice, progressOf, rerunChoices, runCostOf } from './utils';

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** Run detail state: data, the node selection that filters the bottom pane, and actions. */
export function createRunDetailState(runId: string) {
	const topics = [`run:${runId}`, 'runs'];
	// A running run changes about once a second. Its node states are small and reload after each burst;
	// its log, statements, output and cost read the whole run each time, so they reload at most every
	// few seconds, and the page keeps up instead of falling ever further behind.
	const BULKY = { every: 3000 };
	const detailRes = createResource((client, scope) => client.getRunDetail(scope, runId), topics);
	// A group run's own reads are nearly empty, so its log and statements gather its job runs' too.
	// A string, so the reads below reload only when the set of runs changes, not on every detail reload.
	const runKey = $derived([runId, ...(detailRes.data?.children ?? []).map((c) => c.id)].join(','));
	const eachRun = async <T>(read: (id: string) => Promise<T[]>): Promise<T[]> => (await Promise.all(runKey.split(',').map(read))).flat();
	const eventsRes = createResource(
		(client, scope) => (client.supports('listRunEvents') ? eachRun(async (id) => (await client.listRunEvents(scope, id, { limit: 5000 })).events).then((events) => events.sort((a, b) => a.occurredAt.localeCompare(b.occurredAt))) : Promise.resolve([])),
		topics,
		BULKY
	);
	const outputRes = createResource(
		(client, scope) =>
			client.supports('listRunOutput') && app.can('view.evidence')
				? eachRun(async (id) => (await client.listRunOutput(scope, id)).map((line) => ({ ...line, runId: id }))).then((lines) => lines.sort((a, b) => a.at.localeCompare(b.at)))
				: Promise.resolve([]),
		topics,
		BULKY
	);
	const statementsRes = createResource((client, scope) => (client.supports('listRunStatements') ? eachRun((id) => client.listRunStatements(scope, id)) : Promise.resolve([])), topics, BULKY);
	const costRes = createResource((client, scope) => (client.supports('listRunCost') ? eachRun((id) => client.listRunCost(scope, id)) : Promise.resolve([])), topics, BULKY);

	const detail = $derived(detailRes.data ?? null);
	const run = $derived(detail?.run ?? null);
	const nodes = $derived(detail?.nodes ?? []);
	const active = $derived(!!run && isActiveRun(run.status));
	const progress = $derived(progressOf(nodes));
	// ---- the timeline narrowed to one outcome, picked in the panel's summary ----
	let statusFilter = $state<NodeGroupKey | null>(null);
	const nodeGroups = $derived(groupNodes(nodes, active));
	/** The timeline's nodes: those of the outcome picked, else all; a picked outcome no node has any more shows all. */
	const shownNodes = $derived.by(() => {
		const group = statusFilter && nodeGroups.find((g) => g.key === statusFilter);
		return group ? group.nodes : nodes;
	});
	const cause = $derived(outputRes.data?.find((l) => l.error)?.text ?? null);
	const banners = $derived(detail ? bannersFor(detail, app.now, cause) : []);
	const statements = $derived(statementsRes.data ?? []);
	const cost = $derived(costRes.data ? runCostOf(costRes.data) : null);
	// Progress reported by a run with no signal is stale, so the live card only shows while it's running.
	const runningStatements = $derived(run?.status === 'running' ? statements.filter((s) => s.status === 'running') : []);
	let showDetails = $state(false);

	// ---- the selection to plan: the Lineage page's own selection, typed or picked ----
	// It reads like `sqb build -s …`; quick picks fill it, and ticking a node on the timeline or graph adds
	// or removes its name. The timeline, the graph and (by default) the log follow what it selects.
	const selection = createSelection();
	/** The run's nodes the selection selects, by key. */
	const selected = $derived(selection.keys ? nodes.filter((n) => selection.keys!.has(n.nodeKey)).map((n) => n.nodeKey) : []);
	let tab = $state<BottomTab>('log');

	/** Ticking a node adds its name to the selection, or removes it. */
	function selectNode(key: string) {
		selection.select = withNameToggled(selection.select, nodeLabel(key));
	}

	function pick(quick: QuickPick) {
		selection.select = quickPickText(quick, nodes, run?.jobKey ?? null);
		selection.exclude = '';
	}

	function clearSelection() {
		selection.select = '';
		selection.exclude = '';
	}

	// ---- the quick filter: a summary figure's nodes, or a node clicked in the log; apart from the selection ----
	let focus = $state<{ label: string; keys: string[] } | null>(null);

	function focusNode(key: string, nextTab: BottomTab) {
		focus = { label: nodeLabel(key), keys: [key] };
		tab = nextTab;
	}

	/** A summary figure: the timeline and the log show only its nodes; again shows everything. */
	function focusGroup(key: NodeGroupKey | null, label = '') {
		if (!key || statusFilter === key) {
			statusFilter = null;
			focus = null;
			return;
		}
		statusFilter = key;
		const group = nodeGroups.find((g) => g.key === key);
		focus = { label, keys: group?.nodes.map((n) => n.nodeKey) ?? [] };
	}

	// ---- the node drawer ----
	let drawer = $state<string | null>(null);

	const summary = $derived(summaryOf(nodes, eventsRes.data ?? [], active));
	/** Plan these: the plan page for the selection, as the job (or the group) would build it. */
	const planLink = $derived(
		selection.select.trim()
			? planHref(selection.select.trim().split(/\s+/), 'build', { job: run?.kind === 'group' ? undefined : (run?.jobKey ?? undefined), exclude: selection.exclude.trim() ? selection.exclude.trim().split(/\s+/) : [] })
			: null
	);

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
		get nodeGroups() {
			return nodeGroups;
		},
		get shownNodes() {
			return shownNodes;
		},
		get statusFilter() {
			return nodeGroups.some((g) => g.key === statusFilter) ? statusFilter : null;
		},
		/** Shows only the nodes of `key` on the timeline, or all again when it is already picked. */
		filterStatus(key: NodeGroupKey | null) {
			statusFilter = statusFilter === key ? null : key;
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
		selection,
		pick,
		get focus() {
			return focus;
		},
		clearFocus() {
			focus = null;
			statusFilter = null;
		},
		focusGroup,
		get drawer() {
			return drawer;
		},
		openDrawer(key: string | null) {
			drawer = key;
		},
		/** A click on a node's row, bar or card: ticks it in the selection (or unticks it) and shows it in the drawer. */
		clickNode(key: string) {
			selectNode(key);
			drawer = key;
		},
		get summary() {
			return summary;
		},
		get planLink() {
			return planLink;
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
