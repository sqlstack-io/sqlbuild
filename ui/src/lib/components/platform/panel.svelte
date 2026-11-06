<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = { title: string; note?: string; actions?: Snippet; children: Snippet };
	let { title, note, actions, children }: Props = $props();
</script>

<section class="panel">
	<header>
		<h2>{title}</h2>
		{#if note}<span class="note">{note}</span>{/if}
		{#if actions}<div class="actions">{@render actions()}</div>{/if}
	</header>
	{@render children()}
</section>

<style>
	.panel {
		margin-bottom: 28px;
	}
	header {
		display: flex;
		min-height: 30px;
		align-items: center;
		gap: 10px;
		margin-bottom: 8px;
	}
	h2 {
		font-size: 13px;
		font-weight: 600;
	}
	.note {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: auto;
	}
</style>
