<script lang="ts">
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { Check, CheckAttachment, CheckOutcome } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RunPanel from '$lib/run/run-panel.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { checkOutcomeLabel, checkOutcomeTone } from '$lib/status';
	import { attachmentLabel, evaluationModeLabel, runScopePhaseLabel } from '$lib/build-area/labels';
	import { checkValue, outcomeOf } from './utils';

	type OutcomeFilter = 'all' | 'failing' | 'warn' | 'pass' | 'other';

	const checks = createResource((client, scope) => client.listChecks(scope), ['checks', 'runs']);
	const all = $derived(checks.data ?? []);

	let query = $state(page.url.searchParams.get('q') ?? '');
	let outcomeFilter = $state<OutcomeFilter>('all');
	let attachment = $state<CheckAttachment | ''>('');
	let nodeFilter = $state(page.url.searchParams.get('node') ?? '');

	const bucket = (c: Check): Exclude<OutcomeFilter, 'all'> => {
		const o = outcomeOf(c);
		return o === 'fail' || o === 'error' ? 'failing' : o === 'warn' ? 'warn' : o === 'pass' ? 'pass' : 'other';
	};

	const filtered = $derived(
		all.filter((c) => {
			if (outcomeFilter !== 'all' && bucket(c) !== outcomeFilter) return false;
			if (attachment && c.attachment !== attachment) return false;
			if (nodeFilter && c.node !== nodeFilter) return false;
			const q = query.trim().toLowerCase();
			return !q || [c.name, c.node ?? '', c.column ?? '', c.definition ?? c.kind].some((s) => s.toLowerCase().includes(q));
		})
	);
	const counts = $derived({
		all: all.length,
		failing: all.filter((c) => bucket(c) === 'failing').length,
		warn: all.filter((c) => bucket(c) === 'warn').length,
		pass: all.filter((c) => bucket(c) === 'pass').length,
		other: all.filter((c) => bucket(c) === 'other').length
	});
	const outcomeOptions: { value: OutcomeFilter; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'failing', label: 'Failing' },
		{ value: 'warn', label: 'Warning' },
		{ value: 'pass', label: 'Passing' },
		{ value: 'other', label: 'Skipped' }
	];
	const nodes = $derived([...new Set(all.map((c) => c.node).filter((n): n is string => !!n))].sort());

	// A check-only run (`sqb audit`) over the nodes the visible checks attach to.
	const runSelection = $derived([...new Set(filtered.map((c) => c.node).filter((n): n is string => !!n))]);
	let runOpen = $state(false);
</script>

<AppTopbar title="Checks">
	<Button variant="outline" size="sm" onclick={() => checks.refresh()}>↻ Refresh</Button>
	<GatedButton requires="run.adhoc" variant="default" onclick={() => (runOpen = true)}>▷ Run checks</GatedButton>
</AppTopbar>

<RunPanel bind:open={runOpen} verb="audit" selection={filtered.length === all.length ? [] : runSelection} />

<ListToolbar
	bind:query
	placeholder="Search checks"
	total="{counts.pass} of {counts.all} passing"
	counts={[
		{ tone: 'var(--sb-warning)', count: counts.warn, label: 'warning' },
		{ tone: 'var(--sb-error)', count: counts.failing, label: 'failing' }
	]}
>
	{#snippet filters()}
		<SegmentedFilter bind:value={outcomeFilter} options={outcomeOptions.map((o) => ({ ...o, count: counts[o.value] }))} />
		<ChoiceFilter label="Attached to" bind:value={attachment} options={(['model', 'source', 'seed', 'end'] as CheckAttachment[]).map((a) => ({ value: a, label: attachmentLabel[a], count: all.filter((c) => c.attachment === a).length }))} />
		<ChoiceFilter label="Node" bind:value={nodeFilter} options={nodes.map((n) => ({ value: n, label: n, count: all.filter((c) => c.node === n).length }))} />
	{/snippet}
</ListToolbar>

<div class="sb-list-wrap">
	<table class="sb-list">
		<thead>
			<tr>
				<th>Check</th>
				<th>Outcome</th>
				<th>Attached to</th>
				<th>Severity</th>
				<th>Checks</th>
				<th>Result type</th>
				<th class="num">Result</th>
				<th>Recent</th>
				<th>Last checked</th>
			</tr>
		</thead>
		<tbody>
			{#each filtered as c (c.id)}
				{@const o = outcomeOf(c)}
				<tr onclick={() => goto(`/checks/${c.id}`)}>
					<td>
						<div class="cell-name">
							<ShieldCheckIcon size={15} strokeWidth={1.75} />
							<div class="min-w-0">
								<a href="/checks/{c.id}" class="check-name" onclick={(e) => e.stopPropagation()}>{c.name}</a>
								<div class="cell-desc">{c.kind === 'audit' ? `audit · ${c.definition ?? 'custom'}` : c.kind.replace('_', ' ')}</div>
							</div>
						</div>
					</td>
					<td><StatusLabel tone={checkOutcomeTone[o]} label={checkOutcomeLabel[o]} /></td>
					<td>
						{#if c.node}<a href="/catalog/{c.node}" class="link" onclick={(e) => e.stopPropagation()}>{c.node}</a>{#if c.column}<span class="muted">.{c.column}</span>{/if}
						{:else}<span class="muted">{attachmentLabel[c.attachment]}</span>{/if}
					</td>
					<td class="muted capitalize">{c.severity}</td>
					<td class="muted">{runScopePhaseLabel[c.runScopePhase]}</td>
					<td class="muted">{evaluationModeLabel[c.evaluationMode]}</td>
					<td class="num" style:color={o === 'pass' ? 'var(--muted-foreground)' : checkOutcomeTone[o]}>{checkValue(c.latest, c.evaluationMode)}</td>
					<td><span class="mini-strip">{#each c.recentOutcomes as r, i (i)}<i style:background={checkOutcomeTone[r as CheckOutcome]}></i>{/each}</span></td>
					<td><RelativeTime value={formatAgo(c.latest?.evaluatedAt, app.now)} exact={formatStamp(c.latest?.evaluatedAt)} /></td>
				</tr>
			{:else}
				<LoadRow colspan={9} resource={checks} empty={all.length ? 'No checks match the filter.' : 'No checks yet. Add audits or tests to a model and they show up here after the next deploy.'} />
			{/each}
		</tbody>
	</table>
</div>

<style>
	.check-name {
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
</style>
