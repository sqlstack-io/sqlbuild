<script lang="ts">
	import { copyText } from '$lib/browser/clipboard';
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import CheckIcon from '@lucide/svelte/icons/check';
	import HammerIcon from '@lucide/svelte/icons/hammer';
	import MinusIcon from '@lucide/svelte/icons/minus';
	import InfoIcon from '@lucide/svelte/icons/info';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import XIcon from '@lucide/svelte/icons/x';
	import Button from '$lib/components/ui/button/button.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { to } from '$lib/paths';
	import { app } from '$lib/state/app.svelte';
	import { noun } from '$lib/format/plural';
	import { buildHref, copyCommand, problemParts, problemText, problemToken } from '$lib/selection/command';
	import { readSelection } from '$lib/selection/reading';
	import { parseSelection } from '$lib/selection/syntax';
	import type { NameEntry } from '$lib/selection/suggest';
	import type { SelectionState } from '$lib/state/selection.svelte';
	import SelectionField from './selection-field.svelte';

	// The selection bar: what `sqb build -s … --exclude …` would select, typed in SQLBuild's syntax
	// with chips and suggestions, the live node count, the selection in plain words, Copy and Build
	// these. The except row appears once used. `view` holds view-only filters, drawn apart in a
	// dashed group: they narrow what is shown and never join the command.
	let {
		selection,
		names,
		total,
		build = true,
		leading,
		view
	}: {
		selection: SelectionState;
		/** Node names to suggest and pick. */
		names: NameEntry[];
		/** Every node, for the count when nothing narrows. */
		total: number;
		/** Offer Build these. */
		build?: boolean;
		leading?: Snippet;
		view?: Snippet;
	} = $props();

	let exceptOpen = $state(false);
	let copied = $state(false);
	let copyTimer: ReturnType<typeof setTimeout> | undefined;

	const showExcept = $derived(exceptOpen || !!selection.exclude.trim());
	const problem = $derived(selection.problem);
	const shape = (field: 'select' | 'exclude') => {
		const text = field === 'select' ? selection.select : selection.exclude;
		const token = problemToken(problem, field, text);
		if (token === null || !problem) return null;
		return { token, parts: problemParts(problem, parseSelection(text)[token].raw), fix: problem.suggestions[0] ?? null };
	};
	const selectProblem = $derived(shape('select'));
	const exceptProblem = $derived(shape('exclude'));
	const reading = $derived(readSelection(selection.select, selection.exclude));
	const command = $derived(copyCommand({ select: selection.select, exclude: selection.exclude }));
	const canBuild = $derived(app.client.supports('launchAdhocRun'));
	const blocked = $derived(!!problem || !selection.select.trim());

	async function copy() {
		copied = await copyText(command);
		selection.remember();
		clearTimeout(copyTimer);
		copyTimer = setTimeout(() => (copied = false), 1200);
	}
	function buildThese() {
		selection.remember();
		void goto(to(buildHref({ select: selection.select, exclude: selection.exclude })));
	}
	function applyFix() {
		const fix = problem?.suggestions[0];
		const at = problem?.location;
		if (!fix || !at) return;
		const text = at.field === 'select' ? selection.select : selection.exclude;
		const token = problemToken(problem, at.field, text);
		if (token === null) return;
		const tokens = parseSelection(text).map((t) => t.raw);
		tokens.splice(token, 1, fix);
		if (at.field === 'select') selection.select = tokens.join(' ');
		else selection.exclude = tokens.join(' ');
	}
	function clearExcept() {
		selection.exclude = '';
		exceptOpen = false;
	}
</script>

<div class="selection-bar">
	<div class="sb-row">
		{@render leading?.()}
		<SelectionField
			bind:value={selection.select}
			label="Selection"
			placeholder="Select nodes: type, paste sqb syntax, or pick below"
			vocabulary={selection.vocabulary}
			{names}
			recent={selection.recent}
			problem={selectProblem}
			onrecent={(r) => {
				selection.select = r.select;
				selection.exclude = r.exclude;
			}}
			onleave={() => selection.remember()}
		/>
		{#if !showExcept}<Button variant="ghost" size="sm" onclick={() => (exceptOpen = true)} title="Leave nodes out (--exclude)"><MinusIcon />Exclude</Button>{/if}
		<span class="sb-count" class:pending={selection.evaluating} aria-live="polite">
			{#if !selection.active}All <b>{total.toLocaleString('en')}</b> nodes
			{:else if problem}0 nodes
			{:else if selection.evaluation}<b>{selection.evaluation.count.toLocaleString('en')}</b> {noun(selection.evaluation.count, 'node')}
			{:else}—{/if}
		</span>
		<Button variant="outline" size="bar" disabled={blocked} onclick={copy} title={command}>{#if copied}<CheckIcon />Copied{:else}<CopyIcon />Copy as <code>sqb build -s …</code>{/if}</Button>
		{#if build && canBuild}
			<GatedButton requires="run.adhoc" variant="default" disabled={blocked} reason={problem ? 'Fix the selection first' : 'Select nodes first'} hint="Open the plan for {command}" onclick={buildThese}><HammerIcon />Build these</GatedButton>
		{/if}
	</div>
	{#if showExcept || view}
		<div class="sb-row second">
			{#if showExcept}
				<span class="sb-lead">Except</span>
				<SelectionField bind:value={selection.exclude} tone="except" label="Except" placeholder="Leave out: same syntax" vocabulary={selection.vocabulary} {names} problem={exceptProblem} />
				<Button variant="ghost" size="icon-sm" onclick={clearExcept} aria-label="Remove the except row" title="Remove the except row"><XIcon /></Button>
			{/if}
			{#if view}
				<div class="sb-view" title="View filters only change what is drawn. They are not part of the selection, are not copied into the command and do not change what Build these runs.">
					<span class="sb-view-label"><EyeIcon size={13} />View</span>
					{@render view()}
				</div>
			{/if}
		</div>
	{/if}
	{#if problem}
		<div class="sb-msg err" role="alert">
			<CircleAlertIcon size={14} />
			<span title={problem.refused ? problem.message : undefined}>{problemText(problem)}</span>
			{#if problem.suggestions[0]}
				<Button variant="link" size="inline" onclick={applyFix}>Did you mean <code>{problem.suggestions[0]}</code>?</Button>
				<kbd>Tab</kbd><span class="faint">to fix</span>
			{/if}
		</div>
	{:else if selection.failed}
		<div class="sb-msg err" role="alert"><CircleAlertIcon size={14} /><span>{selection.failed}</span></div>
	{:else if selection.active}
		<div class="sb-msg">
			<InfoIcon size={14} />
			<span class="reads">Reads as: {#each reading as s, i (i)}{#if s.strong}<b>{s.text}</b>{:else}{s.text}{/if}{/each}</span>
		</div>
	{/if}
</div>

<style>
	.selection-bar {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--border);
		background: var(--card);
	}
	.sb-row {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		padding: 6px 10px;
	}
	.sb-row.second {
		padding-top: 0;
	}
	.sb-lead {
		flex: none;
		width: 48px;
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 500;
	}
	.sb-row.second :global(.sel-field) {
		flex: 0 1 520px;
	}
	.sb-count {
		flex: none;
		min-width: 80px;
		color: var(--muted-foreground);
		font-size: 12.5px;
		text-align: right;
		white-space: nowrap;
		font-variant-numeric: tabular-nums;
	}
	.sb-count b {
		color: var(--foreground);
		font-weight: 600;
	}
	.sb-count.pending {
		opacity: 0.6;
	}
	.sb-row code {
		font: 12px var(--font-mono);
	}
	.sb-view {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		margin-left: auto;
		padding: 0 4px 0 8px;
		border: 1px dashed var(--border-strong);
		border-radius: 3px;
	}
	.sb-view-label {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-weight: 600;
	}
	.sb-msg {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		padding: 0 12px 7px 14px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.sb-msg b {
		color: var(--foreground);
		font-weight: 500;
	}
	.sb-msg.err {
		color: var(--sb-error);
	}
	.sb-msg.err > span:first-of-type {
		color: var(--foreground);
	}
	.sb-msg code {
		font: 12px var(--font-mono);
	}
	.faint {
		color: var(--muted-foreground);
	}
	kbd {
		padding: 0 4px;
		border: 1px solid var(--border);
		border-radius: 2px;
		font: 11px var(--font-mono);
		color: var(--muted-foreground);
	}
</style>
