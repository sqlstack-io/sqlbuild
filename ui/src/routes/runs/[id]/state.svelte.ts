import { to } from '$lib/paths';
import type { RunEvent, RunNode, RunNodeError } from '$lib/api';
import { untrack } from 'svelte';
import { followEvents, followOutput, newFollow, type RunFollow } from './api/follow';
import { errorMessage } from '$lib/platform/labels';
import { app } from '$lib/state/app.svelte';
import { createResource } from '$lib/state/resource.svelte';
import { createSelection } from '$lib/state/selection.svelte';
import { nodeLabel } from '$lib/operate/labels';
import { planHref } from '$lib/run/command';
import { pageMemory } from './utils/page-memory';
import { quickPickText, runFailures, summaryOf, withNameToggled, type QuickPick } from './utils';
import { isActiveRun } from '$lib/status';
import type { BottomTab, NodeGroupKey } from './types';
import { bannersFor, groupNodes, progressOf, runCostOf } from './utils';

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** Run detail state: data, the node selection that filters the bottom pane, and actions. */
export function createRunDetailState(runId: string) {
	// Back to this page restores it as it was.
	const memory = pageMemory(runId);
	const remembered = <T>(key: string, fallback: T): T => (key in memory ? (memory[key] as T) : fallback);
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
	// Each run's events and output carry on from where the last read stopped, and a run read once
	// after it ended is not read again: its record cannot change.
	const follows = new Map<string, RunFollow>();
	const followOf = (id: string) => follows.get(id) ?? follows.set(id, newFollow()).get(id)!;
	/** The runs whose events and output were complete when a read started: ended, by the detail last read. */
	const endedRuns = () => {
		const d = untrack(() => detailRes.data);
		return new Set([d?.run, ...(d?.children ?? [])].filter((r) => r && !isActiveRun(r.status)).map((r) => r!.id));
	};
	let nodeLogs = $state<RunEvent[]>([]);
	const eventsRes = createResource(
		(client, scope) => {
			if (!client.supports('listRunEvents')) return Promise.resolve([]);
			const ended = endedRuns();
			return eachRun((id) => followEvents(client, scope, followOf(id), { id, ended: ended.has(id) })).then((events) => events.sort((a, b) => a.occurredAt.localeCompare(b.occurredAt)));
		},
		topics,
		BULKY
	);
	const outputRes = createResource(
		(client, scope) => {
			if (!client.supports('listRunOutput') || !app.can('view.evidence')) return Promise.resolve([]);
			const ended = endedRuns();
			return eachRun((id) => followOutput(client, scope, followOf(id), { id, ended: ended.has(id) })).then((lines) => {
				nodeLogs = runKey.split(',').flatMap((id) => followOf(id).logs);
				return lines.sort((a, b) => a.at.localeCompare(b.at));
			});
		},
		topics,
		BULKY
	);
	/** The runs' events with the lines their nodes logged with `ctx.log`, in time order. */
	const events = $derived(nodeLogs.length ? [...(eventsRes.data ?? []), ...nodeLogs].sort((a, b) => a.occurredAt.localeCompare(b.occurredAt)) : (eventsRes.data ?? []));
	const statementsRes = createResource((client, scope) => (client.supports('listRunStatements') ? eachRun((id) => client.listRunStatements(scope, id)) : Promise.resolve([])), topics, BULKY);
	const costRes = createResource((client, scope) => (client.supports('listRunCost') ? eachRun((id) => client.listRunCost(scope, id)) : Promise.resolve([])), topics, BULKY);

	const detail = $derived(detailRes.data ?? null);
	const run = $derived(detail?.run ?? null);
	const nodes = $derived(detail?.nodes ?? []);
	const active = $derived(!!run && isActiveRun(run.status));
	const progress = $derived(progressOf(nodes));
	// ---- the timeline narrowed to one outcome, picked in the panel's summary ----
	let statusFilter = $state<NodeGroupKey | null>(remembered('statusFilter', null));
	/** The header's failed-checks figure: the log shows only failed checks; again shows everything. */
	let checksFilter = $state<boolean>(remembered('checksFilter', false));
	/** The picked figure's words, such as `built` or `failed checks`, for the active-filter line. */
	let figureLabel = $state<string>(remembered('figureLabel', ''));
	const nodeGroups = $derived(groupNodes(nodes, active));
	/** The timeline's nodes: those of the outcome picked, else all; a picked outcome no node has any more shows all. */
	const shownNodes = $derived.by(() => {
		const group = statusFilter && nodeGroups.find((g) => g.key === statusFilter);
		return group ? group.nodes : nodes;
	});
	const cause = $derived(outputRes.data?.find((l) => l.error)?.text ?? null);
	// The failure summary leads when it knows what failed; the worker's own end line is never a reason.
	const housekeeping = (text: string | null) => !!text && /worker executor completed|closed its connections/.test(text);
	const failureCount = $derived(runFailures(nodes, events, statementsRes.data ?? [], run?.target ?? '').length);
	const banners = $derived(
		detail
			? bannersFor(housekeeping(detail.run.statusReason) ? { ...detail, run: { ...detail.run, statusReason: null } } : detail, app.now, housekeeping(cause) ? null : cause).filter((b) => b.key !== 'failed' || !failureCount)
			: []
	);
	const statements = $derived(statementsRes.data ?? []);
	const cost = $derived(costRes.data ? runCostOf(costRes.data) : null);
	// Progress reported by a run with no signal is stale, so the live card only shows while it's running.
	const runningStatements = $derived(run?.status === 'running' ? statements.filter((s) => s.status === 'running') : []);
	let showDetails = $state(false);

	// ---- the selection to plan: the Lineage page's own selection, typed or picked ----
	// It reads like `sqb build -s …`; quick picks fill it, and ticking a node on the timeline or graph adds
	// or removes its name. The timeline, the graph and (by default) the log follow what it selects.
	const selection = createSelection({ initial: { select: remembered('select', ''), exclude: remembered('exclude', '') } });
	/** The run's nodes the selection selects, by key. */
	const selected = $derived(selection.keys ? nodes.filter((n) => selection.keys!.has(n.nodeKey)).map((n) => n.nodeKey) : []);
	let tab = $state<BottomTab>(remembered('tab', 'log'));
	/** The run's nodes the selection leaves out by name (`--exclude`), struck through on the timeline and graph. */
	const excluded = $derived.by(() => {
		const out = new Set(selection.exclude.split(/\s+/).filter(Boolean));
		return nodes.filter((n) => out.has(nodeLabel(n.nodeKey))).map((n) => n.nodeKey);
	});

	/**
	 * Ticking a node always toggles whether it is shown: its plain name comes out of, or goes
	 * into, the selection; a node selected through a pick such as `fct_order_margin+` is excluded instead,
	 * and ticking it again takes the exclude back out.
	 */
	function selectNode(key: string) {
		const name = nodeLabel(key);
		const words = (text: string) => text.split(/\s+/).filter(Boolean);
		if (words(selection.exclude).includes(name)) selection.exclude = withNameToggled(selection.exclude, name);
		else if (words(selection.select).includes(name) || !selected.includes(key)) selection.select = withNameToggled(selection.select, name);
		else selection.exclude = withNameToggled(selection.exclude, name);
	}

	// A quick pick toggles: picked again while its selection is unchanged, it puts back the selection it replaced.
	let picked = $state<{ pick: QuickPick; text: string; select: string; exclude: string } | null>(null);
	function pick(quick: QuickPick) {
		const text = quickPickText(quick, nodes, run?.jobKey ?? null);
		const current = picked && picked.text === selection.select && !selection.exclude ? picked : null;
		if (current?.pick === quick) {
			selection.select = current.select;
			selection.exclude = current.exclude;
			picked = null;
			return;
		}
		// Switching from one pick to another keeps the selection from before the first.
		const before = current ?? { select: selection.select, exclude: selection.exclude };
		picked = { pick: quick, text, select: before.select, exclude: before.exclude };
		selection.select = text;
		selection.exclude = '';
	}
	/** The quick pick whose selection is shown, if it is unchanged since it was picked. */
	const activePick = $derived(picked && picked.text === selection.select && !selection.exclude ? picked.pick : null);

	function clearSelection() {
		selection.select = '';
		selection.exclude = '';
	}

	// ---- the quick filter: a summary figure's nodes, or a node clicked in the log; apart from the selection ----
	// A jump's filter belongs to the tab it opened (`tab`); leaving that tab drops it, so the tab left
	// behind shows what it did before. A summary figure's filter has no tab: it applies everywhere.
	let focus = $state<{ label: string; keys: string[]; tab?: BottomTab } | null>(remembered('focus', null));

	/** A filter for the log, set from outside it (the failure summary's Show in log); `seq` makes a repeat apply again. */
	let logRequest = $state<{ query: string; seq: number } | null>(null);
	function showInLog(query: string) {
		focus = null;
		statusFilter = null;
		tab = 'log';
		logRequest = { query, seq: (logRequest?.seq ?? 0) + 1 };
	}

	function focusNode(key: string, nextTab: BottomTab) {
		focus = { label: nodeLabel(key), keys: [key], tab: nextTab };
		tab = nextTab;
	}

	function switchTab(next: BottomTab) {
		if (focus?.tab && focus.tab !== next) focus = null;
		if (next !== 'statements') statementFocus = null;
		tab = next;
	}

	/** A summary figure: the timeline and the log show only its nodes; again shows everything. */
	// A header figure toggles: on keeps the focus it replaced (a node picked in the log), off puts it back.
	let focusBefore: typeof focus | undefined = undefined;
	function figureOn() {
		if (focusBefore === undefined && !statusFilter && !checksFilter) focusBefore = focus;
	}
	function figureOff() {
		statusFilter = null;
		checksFilter = false;
		focus = focusBefore ?? null;
		focusBefore = undefined;
	}
	function focusGroup(key: NodeGroupKey | null, label = '') {
		if (!key || statusFilter === key) return figureOff();
		figureOn();
		checksFilter = false;
		figureLabel = label;
		statusFilter = key;
		const group = nodeGroups.find((g) => g.key === key);
		focus = { label, keys: group?.nodes.map((n) => n.nodeKey) ?? [] };
	}

	// ---- failed nodes' error text ----
	// An audited evidence read: once per node, and only when the error is shown.
	const errorsReadable = $derived(app.client.supports('getRunNodeError') && app.can('view.evidence'));
	let nodeErrors = $state<Record<string, RunNodeError | { failure: string }>>({});
	const errorsAsked = new Set<string>();
	async function loadNodeError(n: RunNode) {
		const k = `${n.runId}/${n.nodeKey}`;
		if (!errorsReadable || n.errorDetail || errorsAsked.has(k) || !app.scope) return;
		errorsAsked.add(k);
		try {
			nodeErrors[k] = await app.client.getRunNodeError(app.scope, n.runId, n.nodeKey);
		} catch (error) {
			nodeErrors[k] = { failure: errorMessage(error) };
		}
	}

	// ---- the node drawer ----
	let drawer = $state<string | null>(remembered('drawer', null));
	/** The statement a check's "In Statements" opened. */
	let statementFocus = $state<string | null>(null);
	/** A line of the drawer's node's code to show, from a traceback frame. */
	let codeLine = $state<number | null>(remembered('codeLine', null));
	$effect(() => {
		Object.assign(memory, { select: selection.select, exclude: selection.exclude, tab, focus: $state.snapshot(focus), statusFilter, checksFilter, figureLabel, drawer, codeLine });
	});

	const summary = $derived(summaryOf(nodes, events, active));
	/** Plan these: the plan page for the selection, as the job (or the group) would build it. */
	const planLink = $derived(
		selection.select.trim()
			? planHref(selection.select.trim().split(/\s+/), 'build', { job: run?.kind === 'group' ? undefined : (run?.jobKey ?? undefined), exclude: selection.exclude.trim() ? selection.exclude.trim().split(/\s+/) : [] })
			: null
	);

	// ---- actions ----
	let busy = $state<'cancel' | null>(null);
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

	return {
		detailRes,
		eventsRes,
		get events() {
			return events;
		},
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
		/** The header figure whose filter is on: a node group, or the failed checks. */
		get figure(): NodeGroupKey | 'checks_failed' | null {
			if (checksFilter) return 'checks_failed';
			return nodeGroups.some((g) => g.key === statusFilter) ? statusFilter : null;
		},
		get figureLabel() {
			return figureLabel;
		},
		get checksFilter() {
			return checksFilter;
		},
		/** The failed-checks figure: on shows only failed checks in the log; again shows everything. */
		toggleChecksFilter(label: string) {
			if (checksFilter) return figureOff();
			figureOn();
			statusFilter = null;
			focus = null;
			figureLabel = label;
			checksFilter = true;
			tab = 'log';
		},
		get activePick() {
			return activePick;
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
			if (statusFilter || checksFilter) return figureOff();
			focus = null;
		},
		focusGroup,
		showInLog,
		get logRequest() {
			return logRequest;
		},
		get errorsReadable() {
			return errorsReadable;
		},
		loadNodeError,
		/** The error read for a failed node, once it has been. */
		nodeError(n: RunNode): RunNodeError | null {
			const found = nodeErrors[`${n.runId}/${n.nodeKey}`];
			return found && 'nodeKey' in found ? found : null;
		},
		/** Why a failed node's error could not be read. */
		nodeErrorFailure(n: RunNode): string | null {
			const found = nodeErrors[`${n.runId}/${n.nodeKey}`];
			return found && 'failure' in found ? found.failure : null;
		},
		get drawer() {
			return drawer;
		},
		openDrawer(key: string | null) {
			drawer = key;
			if (key) showDetails = false;
			codeLine = null;
		},
		get codeLine() {
			return codeLine;
		},
		/** A traceback frame in a node's own code: the drawer, at that line of its code. */
		openCode(key: string, line: number) {
			drawer = key;
			codeLine = line;
		},
		get excluded() {
			return excluded;
		},
		/** What this page remembers for Back: components keep their own parts here too. */
		memory,
		get statementFocus() {
			return statementFocus;
		},
		/** A check's query: the Statements tab, filtered to its node, with that statement open. */
		showStatement(statementId: string, nodeKey: string) {
			focus = { label: nodeLabel(nodeKey), keys: [nodeKey], tab: 'statements' };
			statementFocus = statementId;
			tab = 'statements';
		},
		/** A click on a node's row, bar or card: ticks it in the selection (or unticks it) and shows it in the drawer. */
		clickNode(key: string) {
			selectNode(key);
			drawer = key;
			showDetails = false;
			codeLine = null;
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
		/** Details and the node drawer share the right-hand panel: opening one closes the other. */
		toggleDetails() {
			showDetails = !showDetails;
			if (showDetails) drawer = null;
		},
		get tab() {
			return tab;
		},
		set tab(value: BottomTab) {
			switchTab(value);
		},
		selectNode,
		clearSelection,
		focusNode,
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
		cancel
	};
}

export type RunDetailState = ReturnType<typeof createRunDetailState>;
