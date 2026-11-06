<script lang="ts">
	import CopyButton from '$lib/components/copy-button.svelte';
	import type { RunNode } from '$lib/api';
	import { shortRunId } from '$lib/operate/labels';
	import type { RunDetailState } from './state.svelte';

	let { view }: { view: RunDetailState } = $props();

	const failed = $derived(view.nodes.filter((n) => n.status === 'failed'));
	const skipped = $derived(view.nodes.filter((n) => n.status === 'skipped').length);
	let open = $state<Set<string>>(new Set());
	let collapsed = $state(false);

	function toggle(n: RunNode) {
		const key = `${n.runId}/${n.nodeKey}`;
		const next = new Set(open);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		open = next;
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
					<li>
						<div class="row">
							<button class="node mono" onclick={() => view.focusNode(n.nodeKey, 'statements')} title="Show this node's queries below">{n.nodeKey}</button>
							{#if n.runId !== view.run?.id}<a class="faint" href="/runs/{n.runId}">in {n.jobKey ?? shortRunId(n.runId)}</a>{/if}
							<span class="msg">{n.errorMessage ?? 'Failed'}</span>
							{#if n.errorLocation}
								<span class="loc mono" title="Where the failure points in the project">{n.errorLocation.file}:{n.errorLocation.line}</span>
								<CopyButton value="{n.errorLocation.file}:{n.errorLocation.line}" label="Copy file and line" />
							{/if}
							{#if n.errorDetail}
								<button class="more" onclick={() => toggle(n)} aria-expanded={open.has(key)}>{open.has(key) ? 'Hide details' : 'Details'}</button>
							{/if}
						</div>
						{#if open.has(key) && n.errorDetail}
							<pre>{n.errorDetail}</pre>
							{#if n.errorCode}<p class="faint">Error code <code>{n.errorCode}</code></p>{/if}
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
