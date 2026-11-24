<script lang="ts">
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import CopyButton from '$lib/components/copy-button.svelte';
	import type { RunNode } from '$lib/api';
	import { shortRunId } from '$lib/operate/labels';
	import { errorHeadline, errorTextOf, tracebackOf } from './utils';
	import type { RunDetailState } from './state.svelte';
	import QueryPanel from './query-panel.svelte';
	import TracebackPanel from './traceback-panel.svelte';

	let { view }: { view: RunDetailState } = $props();

	const failed = $derived(view.nodes.filter((n) => n.status === 'failed'));
	const skipped = $derived(view.nodes.filter((n) => n.status === 'skipped').length);
	let open = $state<Set<string>>(new Set());
	let listOpen = $state(false);

	function toggle(n: RunNode) {
		const key = `${n.runId}/${n.nodeKey}`;
		const next = new Set(open);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		open = next;
		if (next.has(key)) void view.loadNodeError(n);
	}

	// Collapsed by default: one line per failure, opening to where it points and its text.
	const single = $derived(failed.length === 1);

	function place(n: RunNode): string | null {
		if (n.errorLocation) return n.errorLocation.line === null ? n.errorLocation.file : `${n.errorLocation.file}:${n.errorLocation.line}`;
		return view.nodeError(n)?.file ?? null;
	}
</script>

{#snippet line(n: RunNode)}
	{@const key = `${n.runId}/${n.nodeKey}`}
	{@const loaded = view.nodeError(n)}
	{@const where = place(n)}
	{@const detail = errorTextOf(n, loaded)}
	{@const traceback = tracebackOf(detail)}
	<li>
		<!-- raw-button: the failure's one line, opening its location and text -->
		<button class="row" onclick={() => toggle(n)} aria-expanded={open.has(key)} title={open.has(key) ? 'Hide the error' : 'Show where it failed and the error'}>
			<CircleXIcon class="x" size={14} aria-hidden="true" />
			<span class="node mono">{n.nodeKey}</span>
			{#if n.runId !== view.run?.id}<span class="faint">in {n.jobKey ?? shortRunId(n.runId)}</span>{/if}
			<span class="msg">{n.errorMessage ?? errorHeadline(loaded?.message) ?? (n.errorCode ? `Failed with ${n.errorCode}` : 'Failed')}</span>
			<ChevronRightIcon class={open.has(key) ? 'caret open' : 'caret'} size={14} aria-hidden="true" />
		</button>
		{#if open.has(key)}
			{@const failure = view.nodeErrorFailure(n)}
			{@const failedQueries = view.statements.filter((q) => q.nodeKey === n.nodeKey && q.runId === n.runId && q.status === 'failed')}
			<div class="more">
				{#if where}
					<div class="where"><span class="loc mono" title="Where the failure points in the project">{where}</span><CopyButton value={where} label="Copy file" />
						{#if n.runId !== view.run?.id}<a class="faint" href={to(`/runs/${n.runId}`)}>Open its job run</a>{/if}
					</div>
				{/if}
				<!-- The failure in place: a Python exception's traceback, or the failed query and its error. -->
				{#if traceback}
					<TracebackPanel detail={traceback} file={n.errorLocation?.file ?? null} runId={n.runId} nodeKey={n.nodeKey} onframe={(line) => view.openCode(n.nodeKey, line)} />
				{:else if failedQueries.length}
					{#each failedQueries as q (q.id)}<QueryPanel statement={q} onstatements={() => view.showStatement(q.id, n.nodeKey)} />{/each}
				{:else if detail}
					<pre>{detail}</pre>
				{:else if failure}
					<p class="faint">{failure}</p>
				{:else if loaded}
					<p class="faint">No error text recorded.</p>
				{:else if view.errorsReadable}
					<p class="faint">Loading the error…</p>
				{/if}
			</div>
		{/if}
	</li>
{/snippet}

{#if failed.length}
	<section class="failures" aria-label="Failures">
		{#if single}
			<ul>{@render line(failed[0])}</ul>
		{:else}
			<!-- raw-button: the failures' one line, opening the list -->
			<button class="row head" onclick={() => (listOpen = !listOpen)} aria-expanded={listOpen}>
				<CircleXIcon class="x" size={14} aria-hidden="true" /><b>{failed.length} failed</b>
				{#if skipped}<span class="faint">· {skipped} skipped because of them</span>{/if}
				<ChevronRightIcon class={listOpen ? 'caret open' : 'caret'} size={14} aria-hidden="true" />
			</button>
			{#if listOpen}
				<ul class="nested">
					{#each failed as n (`${n.runId}/${n.nodeKey}`)}{@render line(n)}{/each}
				</ul>
			{/if}
		{/if}
	</section>
{/if}

<style>
	.failures {
		flex: 0 0 auto;
		border-bottom: 1px solid var(--border-subtle);
		background: color-mix(in srgb, var(--sb-error) 2.5%, var(--background));
		font-size: 12.5px;
	}
	ul {
		display: flex;
		flex-direction: column;
		max-height: 260px;
		overflow: auto;
	}
	ul.nested {
		padding-left: 18px;
	}
	.row {
		display: flex;
		width: 100%;
		min-width: 0;
		align-items: center;
		gap: 8px;
		padding: 5px 24px;
		text-align: left;
	}
	.row:hover {
		background: color-mix(in srgb, var(--sb-error) 6%, transparent);
	}
	.head b {
		color: var(--sb-error);
		font-weight: 600;
	}
	.row :global(.x) {
		flex: none;
		color: var(--sb-error);
	}
	.row :global(.caret) {
		flex: none;
		color: var(--sb-text-faint);
		transition: transform 0.12s;
	}
	.row :global(.caret.open) {
		transform: rotate(90deg);
	}
	.more {
		padding: 0 24px 8px 44px;
	}
	.where {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.node {
		color: var(--sb-error);
		font-weight: 500;
	}
	.msg {
		overflow: hidden;
		min-width: 0;
		flex: 0 1 auto;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.loc {
		color: var(--muted-foreground);
	}
	.faint {
		color: var(--muted-foreground);
	}
	a.faint:hover {
		color: var(--primary);
	}
	pre {
		margin: 6px 0 4px;
		padding: 8px 10px;
		overflow: auto;
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--sb-mono-err);
		font-family: var(--font-mono);
		font-size: 11.5px;
		white-space: pre;
	}
</style>
