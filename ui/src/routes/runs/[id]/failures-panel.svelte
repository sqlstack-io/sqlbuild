<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';
	import { app } from '$lib/state/app.svelte';
	import CopyButton from '$lib/components/copy-button.svelte';
	import type { RunNode, RunNodeError } from '$lib/api';
	import { shortRunId } from '$lib/operate/labels';
	import { errorMessage } from '$lib/platform/labels';
	import type { RunDetailState } from './state.svelte';
	import QueryPanel from './query-panel.svelte';
	import TracebackPanel from './traceback-panel.svelte';

	let { view }: { view: RunDetailState } = $props();

	const failed = $derived(view.nodes.filter((n) => n.status === 'failed'));
	const skipped = $derived(view.nodes.filter((n) => n.status === 'skipped').length);
	// Error text is an audited evidence read, so it is fetched once per node and only when asked for.
	const readable = $derived(app.client.supports('getRunNodeError') && app.can('view.evidence'));
	let open = $state<Set<string>>(new Set());
	let listOpen = $state(false);
	let errors = $state<Record<string, RunNodeError | { failure: string } | undefined>>({});

	async function toggle(n: RunNode) {
		const key = `${n.runId}/${n.nodeKey}`;
		const next = new Set(open);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		open = next;
		if (!next.has(key) || n.errorDetail || errors[key] || !app.scope) return;
		try {
			errors[key] = await app.client.getRunNodeError(app.scope, n.runId, n.nodeKey);
		} catch (error) {
			errors[key] = { failure: errorMessage(error) };
		}
	}

	// Collapsed by default (CHI-1591, change 2): one line per failure, opening to where it points and its text.
	const single = $derived(failed.length === 1);

	function read(key: string): RunNodeError | null {
		const found = errors[key];
		return found && 'nodeKey' in found ? found : null;
	}

	function place(n: RunNode, key: string): string | null {
		if (n.errorLocation) return n.errorLocation.line === null ? n.errorLocation.file : `${n.errorLocation.file}:${n.errorLocation.line}`;
		return read(key)?.file ?? null;
	}
</script>

{#snippet line(n: RunNode)}
	{@const key = `${n.runId}/${n.nodeKey}`}
	{@const loaded = read(key)}
	{@const where = place(n, key)}
	{@const detail = n.errorDetail ?? loaded?.message ?? null}
	<li>
		<!-- raw-button: the failure's one line, opening its location and text -->
		<button class="row" onclick={() => toggle(n)} aria-expanded={open.has(key)} title={open.has(key) ? 'Hide the error' : 'Show where it failed and the error'}>
			<span class="x">✗</span>
			<span class="node mono">{n.nodeKey}</span>
			{#if n.runId !== view.run?.id}<span class="faint">in {n.jobKey ?? shortRunId(n.runId)}</span>{/if}
			<span class="msg">{n.errorMessage ?? loaded?.message?.split('\n')[0] ?? (n.errorCode ? `Failed with ${n.errorCode}` : 'Failed')}</span>
			<span class="caret" class:open={open.has(key)}>▸</span>
		</button>
		{#if open.has(key)}
			{@const got = errors[key]}
			{@const failedQueries = view.statements.filter((q) => q.nodeKey === n.nodeKey && q.runId === n.runId && q.status === 'failed')}
			<div class="more">
				{#if where}
					<div class="where"><span class="loc mono" title="Where the failure points in the project">{where}</span><CopyButton value={where} label="Copy file" />
						{#if n.runId !== view.run?.id}<a class="faint" href={to(`/runs/${n.runId}`)}>Open its job run</a>{/if}
					</div>
				{/if}
				<!-- The failure in place (CHI-1591): a Python exception's traceback, or the failed query and its error. -->
				{#if n.errorCode === 'python_exception' && n.errorDetail}
					<TracebackPanel detail={n.errorDetail} file={n.errorLocation?.file ?? null} runId={n.runId} nodeKey={n.nodeKey} onframe={(line) => view.openCode(n.nodeKey, line)} />
				{:else if failedQueries.length}
					{#each failedQueries as q (q.id)}<QueryPanel statement={q} onstatements={() => view.showStatement(q.id, n.nodeKey)} />{/each}
				{:else if detail}
					<pre>{detail}</pre>
				{:else if got && 'failure' in got}
					<p class="faint">{got.failure}</p>
				{:else if got}
					<p class="faint">No error text was recorded for this node.</p>
				{:else if !n.errorDetail && readable}
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
				<span class="x">✗</span><b>{failed.length} failed</b>
				{#if skipped}<span class="faint">· {skipped} skipped because of them</span>{/if}
				<span class="caret" class:open={listOpen}>▸</span>
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
	.x {
		color: var(--sb-error);
		font-weight: 600;
	}
	.caret {
		color: var(--sb-text-faint);
		transition: transform 0.12s;
	}
	.caret.open {
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
