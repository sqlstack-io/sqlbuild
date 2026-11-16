<script lang="ts">
	import type { Snippet } from 'svelte';
	import { MAX_DRAWN } from '$lib/lineage/folder-graph';

	// Shown over a graph instead of laying out more cards than the browser can, with ways to draw fewer.
	let { count, actions }: { count: number; actions?: Snippet } = $props();
</script>

<div class="graph-too-many">
	<div class="too-many-card" role="status">
		<strong>Too many nodes to draw at once</strong>
		<p>This view has {count.toLocaleString('en')} cards; the graph draws at most {MAX_DRAWN.toLocaleString('en')}.</p>
		{#if actions}<div class="too-many-actions">{@render actions()}</div>{/if}
	</div>
</div>

<style>
	.graph-too-many {
		position: absolute;
		inset: 0;
		z-index: 5;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
		background: var(--sb-inset);
		color: var(--muted-foreground);
		font-size: 13px;
		text-align: center;
	}
	.too-many-card {
		max-width: 420px;
		padding: 20px 22px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--sb-elev);
		color: var(--muted-foreground);
	}
	.too-many-card strong {
		display: block;
		color: var(--foreground);
		font-size: 14px;
		font-weight: 600;
	}
	.too-many-card p {
		margin: 6px 0 14px;
		font-size: 12.5px;
	}
	.too-many-actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 8px;
	}
</style>
