<script lang="ts">
	import { to } from '$lib/paths';
	import { untrack } from 'svelte';
	import { app } from '$lib/state/app.svelte';
	import CopyButton from '$lib/components/copy-button.svelte';
	import type { RunNode, RunNodeError } from '$lib/api';
	import { shortRunId } from '$lib/operate/labels';
	import { errorMessage } from '$lib/platform/labels';
	import type { RunDetailState } from './state.svelte';

	let { view }: { view: RunDetailState } = $props();

	const failed = $derived(view.nodes.filter((n) => n.status === 'failed'));
	const skipped = $derived(view.nodes.filter((n) => n.status === 'skipped').length);
	// Error text is an audited evidence read, so it is fetched once per node and only when asked for.
	const readable = $derived(app.client.supports('getRunNodeError') && app.can('view.evidence'));
	let open = $state<Set<string>>(new Set());
	let collapsed = $state(false);
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

	// The first failure opens on its own, so the error is on screen without a click: one audited read per visit.
	let opened = false;
	$effect(() => {
		const first = failed[0];
		if (opened || !first || !(first.errorDetail || readable)) return;
		opened = true;
		untrack(() => void toggle(first));
	});

	function read(key: string): RunNodeError | null {
		const found = errors[key];
		return found && 'nodeKey' in found ? found : null;
	}

	function place(n: RunNode, key: string): string | null {
		if (n.errorLocation) return n.errorLocation.line === null ? n.errorLocation.file : `${n.errorLocation.file}:${n.errorLocation.line}`;
		return read(key)?.file ?? null;
	}
</script>

{#if failed.length}
	<section class="failures" aria-label="Failures">
		<button class="head" onclick={() => (collapsed = !collapsed)} aria-expanded={!collapsed}>
			<span class="caret">{collapsed ? '▸' : '▾'}</span>
			<b>{failed.length} node{failed.length === 1 ? '' : 's'} failed</b>
			{#if skipped}<span class="faint">· {skipped} skipped because of {failed.length === 1 ? 'it' : 'them'}</span>{/if}
		</button>
		{#if !collapsed}
			<ul>
				{#each failed as n (`${n.runId}/${n.nodeKey}`)}
					{@const key = `${n.runId}/${n.nodeKey}`}
					{@const loaded = read(key)}
					{@const where = place(n, key)}
					{@const detail = n.errorDetail ?? loaded?.message ?? null}
					<li>
						<div class="row">
							{#if app.client.supports('listRunStatements')}<button class="node mono" onclick={() => view.focusNode(n.nodeKey, 'statements')} title="Show this node's queries below">{n.nodeKey}</button>{:else}<span class="node mono">{n.nodeKey}</span>{/if}
							{#if n.runId !== view.run?.id}<a class="faint" href={to(`/runs/${n.runId}`)}>in {n.jobKey ?? shortRunId(n.runId)}</a>{/if}
							<span class="msg">{n.errorMessage ?? loaded?.message?.split('\n')[0] ?? (n.errorCode ? `Failed with ${n.errorCode}` : 'Failed')}</span>
							{#if where}
								<span class="loc mono" title="Where the failure points in the project">{where}</span>
								<CopyButton value={where} label="Copy file" />
							{/if}
							{#if n.errorDetail || readable}
								<button class="more" onclick={() => toggle(n)} aria-expanded={open.has(key)}>{open.has(key) ? 'Hide details' : 'Details'}</button>
							{/if}
						</div>
						{#if open.has(key)}
							{@const got = errors[key]}
							{#if detail}
								<pre>{detail}</pre>
							{:else if got && 'failure' in got}
								<p class="faint">{got.failure}</p>
							{:else if got}
								<p class="faint">No error text was recorded for this node.</p>
							{:else if !n.errorDetail}
								<p class="faint">Loading the error…</p>
							{/if}
							{#if n.errorCode && detail}<p class="faint">Error code <code>{n.errorCode}</code></p>{/if}
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</section>
{/if}

<style>
	.failures {
		flex: 0 0 auto;
		border-bottom: 1px solid color-mix(in srgb, var(--sb-error) 30%, var(--border));
		background: color-mix(in srgb, var(--sb-error) 4%, var(--background));
		font-size: 12.5px;
	}
	.head {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 6px;
		padding: 8px 24px 4px;
		text-align: left;
	}
	.head b {
		color: var(--sb-error);
		font-weight: 600;
	}
	.caret {
		width: 12px;
		color: var(--sb-text-faint);
	}
	ul {
		display: flex;
		flex-direction: column;
		padding: 0 24px 8px 42px;
		max-height: 220px;
		overflow: auto;
	}
	li {
		padding: 3px 0;
	}
	.row {
		display: flex;
		min-width: 0;
		align-items: center;
		gap: 8px;
	}
	.mono,
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.node {
		color: var(--sb-error);
		font-weight: 500;
	}
	.node:hover {
		text-decoration: underline;
	}
	.msg {
		overflow: hidden;
		min-width: 0;
		flex: 1;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.loc {
		color: var(--muted-foreground);
	}
	.more {
		color: var(--primary);
		font-size: 12px;
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
