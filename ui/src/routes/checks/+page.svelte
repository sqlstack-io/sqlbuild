<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { openRow, to } from '$lib/paths';
	import PlayIcon from '@lucide/svelte/icons/play';
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { Check, CheckAttachment, CheckOutcome } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { planHref } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { useChecks } from '$lib/state/checks.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeLabel, checkOutcomeTone } from '$lib/status';
	import { attachmentLabel, evaluationModeHelp, evaluationModeLabel, runScopePhaseHelp, runScopePhaseLabel } from '$lib/build-area/labels';
	import { checkValue, ignoredFailure, kindLabel, outcomeOf, parseOutcomeFilter, type OutcomeFilter } from './utils';
	import { countBy } from '$lib/list/count-by';
	import { matchesAny, readValues } from '$lib/list/choice';
	import SelectionBar from '$lib/components/selection/selection-bar.svelte';
	import { createSelection } from '$lib/state/selection.svelte';
	import { readSelectionParams } from '$lib/selection/command';

	const checks = useChecks();
	const all = $derived(checks.data ?? []);

	let query = $state(page.url.searchParams.get('q') ?? '');
	let outcomeFilter = $state<OutcomeFilter>(parseOutcomeFilter(page.url.searchParams.get('outcome')));
	let attachment = $state<CheckAttachment[]>([]);
	// The nodes checked: a selection, as `sqb` reads one. Older links name nodes with `?node=a&node=b`;
	// a bare name selects that node, so they become the selection.
	const linked = readSelectionParams(page.url.searchParams);
	const selection = createSelection({ url: true, initial: linked.select || linked.exclude ? linked : { select: readValues(page.url.searchParams, 'node').join(' '), exclude: '' }, replaces: ['node'] });
	const attachmentSet = $derived(new Set(attachment));

	const bucket = (c: Check): Exclude<OutcomeFilter, 'all' | 'ignored'> => {
		const o = outcomeOf(c);
		return o === 'fail' || o === 'error' ? 'failing' : o === 'warn' ? 'warn' : o === 'pass' ? 'pass' : 'other';
	};

	const filtered = $derived(
		all.filter((c) => {
			if (outcomeFilter === 'ignored' ? !ignoredFailure(c) : outcomeFilter !== 'all' && bucket(c) !== outcomeFilter) return false;
			if (!matchesAny(attachmentSet, c.attachment)) return false;
			if (!selection.hasName(c.node)) return false;
			const q = query.trim().toLowerCase();
			return !q || [c.name, c.node ?? '', c.column ?? '', kindLabel(c)].some((s) => s.toLowerCase().includes(q));
		})
	);
	const byBucket = $derived(countBy(all, bucket));
	const counts = $derived({
		all: all.length,
		failing: byBucket.get('failing') ?? 0,
		ignored: all.filter(ignoredFailure).length,
		warn: byBucket.get('warn') ?? 0,
		pass: byBucket.get('pass') ?? 0,
		other: byBucket.get('other') ?? 0
	});
	const byAttachment = $derived(countBy(all, (c) => c.attachment));
	const byNode = $derived(countBy(all, (c) => c.node));
	const outcomeOptions: { value: OutcomeFilter; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'failing', label: 'Failing' },
		{ value: 'ignored', label: 'Ignored failures' },
		{ value: 'warn', label: 'Warning' },
		{ value: 'pass', label: 'Passing' },
		{ value: 'other', label: 'Skipped' }
	];
	const nodes = $derived([...new Set(all.map((c) => c.node).filter((n): n is string => !!n))].sort());

	// A check-only run (`sqb audit`) over the nodes the visible checks attach to.
	const runSelection = $derived([...new Set(filtered.map((c) => c.node).filter((n): n is string => !!n))]);
	const linksNodes = $derived(app.client.supports('getCatalogNode'));
	const hasRecent = $derived(all.some((c) => c.recentOutcomes !== null));

	const ROW_H = 52;
	const tableColumns = $derived<TableColumn[]>([
		// Check and Attached to share what is left; together the columns fit a 1,440 px window without scrolling sideways.
		{ id: 'check', label: 'Check', minWidth: 200 },
		{ id: 'outcome', label: 'Outcome', width: '160px' },
		{ id: 'node', label: 'Attached to', minWidth: 160 },
		{ id: 'severity', label: 'Severity', width: '100px' },
		{ id: 'phase', label: 'Scope', width: '150px' },
		{ id: 'mode', label: 'Result type', width: '120px' },
		{ id: 'result', label: 'Result', width: '100px', align: 'right' },
		...(hasRecent ? [{ id: 'recent', label: 'Recent', width: '100px' }] : []),
		{ id: 'checked', label: 'Last checked', width: '120px' }
	]);
</script>

<AppTopbar title="Checks">
	<RefreshButton onclick={() => checks.refresh()} />
	{#if app.client.supports('launchAdhocRun')}<GatedButton size="bar" requires="run.adhoc" variant="default" onclick={() => goto(to(planHref(filtered.length === all.length ? [] : runSelection, 'audit')))}><PlayIcon class="size-3.5 fill-current stroke-none" />Run checks</GatedButton>{/if}
</AppTopbar>


<SelectionBar {selection} names={nodes.map((n) => ({ value: n, hint: '', count: byNode.get(n) ?? 0 }))} total={nodes.length} build={false} />
<ListToolbar
	loading={checks.data === undefined}
	bind:query
	placeholder="Search checks"
	total="{counts.pass} of {counts.all} passing"
	counts={[
		{ tone: 'var(--sb-warning)', count: counts.warn, label: 'warning' },
		{ tone: 'var(--sb-error)', count: counts.failing, label: 'failing' }
	]}
>
	{#snippet filters()}
		<SegmentedFilter loading={checks.data === undefined} bind:value={outcomeFilter} options={outcomeOptions.map((o) => ({ ...o, count: counts[o.value] }))} />
		<ChoiceFilter multiple label="Attached to" bind:values={attachment} options={(['model', 'source', 'seed', 'end'] as CheckAttachment[]).map((a) => ({ value: a, label: attachmentLabel[a], count: byAttachment.get(a) ?? 0 }))} />
	{/snippet}
</ListToolbar>

<DataTable class="checks" columns={tableColumns} rows={filtered} key={(c) => c.id} rowHeight={ROW_H} resource={checks} empty={all.length ? 'No checks match the filter.' : 'No checks yet.'}>
	{#snippet row(c)}
				{@const o = outcomeOf(c)}
				<tr onclick={(e) => openRow(e, `/checks/${c.id}`)}>
					<td>
						<div class="cell-name">
							<ShieldCheckIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								<a href={to(`/checks/${c.id}`)} class="check-name" title={c.name}>{c.name}</a>
								<div class="cell-desc">{kindLabel(c)}</div>
							</div>
						</div>
					</td>
					<td>
						<StatusLabel tone={checkOutcomeTone[o]} label={checkOutcomeLabel[o]} />
						{#if ignoredFailure(c)}<span class="ignored" title="Its build let this failure through instead of stopping: audit failures don't stop that job">ignored</span>{/if}
					</td>
					<td title={c.node ? `${c.node}${c.column ? `.${c.column}` : ''}` : undefined}>
						{#if c.node}{#if linksNodes}<a href={to(`/catalog/${c.node}`)} class="link">{c.node}</a>{:else}<span>{c.node}</span>{/if}{#if c.column}<span class="muted">.{c.column}</span>{/if}
						{:else}<span class="muted">{attachmentLabel[c.attachment]}</span>{/if}
					</td>
					<td class="muted capitalize">{c.severity}</td>
					<td class="muted" title={runScopePhaseHelp[c.runScopePhase]}>{runScopePhaseLabel[c.runScopePhase]}</td>
					<td class="muted" title={evaluationModeHelp[c.evaluationMode]}>{evaluationModeLabel[c.evaluationMode]}</td>
					<td class="num" style:color={o === 'pass' ? 'var(--muted-foreground)' : checkOutcomeTone[o]}>{checkValue(c.latest, c.evaluationMode)}</td>
					{#if hasRecent}<td><span class="mini-strip">{#each c.recentOutcomes ?? [] as r, i (i)}<i style:background={checkOutcomeTone[r as CheckOutcome]}></i>{/each}</span></td>{/if}
					<td><RelativeTime value={formatAgo(c.latest?.evaluatedAt, app.now)} exact={formatStamp(c.latest?.evaluatedAt)} /></td>
				</tr>
	{/snippet}
</DataTable>

<style>
	.check-name {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--foreground);
		font: 12px var(--font-mono);
	}
	.check-name:hover {
		color: var(--primary);
	}
	.mini-strip {
		display: inline-flex;
		gap: 1px;
		height: 12px;
	}
	.mini-strip i {
		width: 4px;
		border-radius: 1px;
		opacity: 0.8;
	}
	.ignored {
		margin-left: 6px;
		padding: 0 6px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--sb-warning) 14%, transparent);
		color: color-mix(in srgb, var(--sb-warning) 75%, var(--foreground));
		font-size: 11px;
	}
</style>
