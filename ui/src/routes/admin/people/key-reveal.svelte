<script lang="ts">
	import { createCopyState } from '$lib/browser/copy-state.svelte';
	import Button from '$lib/components/ui/button/button.svelte';

	// A key shown once in a dialog: the key, Copy, and the warning that it won't be shown again.
	let { secret }: { secret: string } = $props();

	const copier = createCopyState();
	const copied = $derived(copier.copied);
	// Copying can fail where the browser blocks it; the key is then selected to copy by hand.
	const failed = $derived(copier.failed);
	let shown = $state<HTMLElement>();
	async function copy(e: MouseEvent) {
		const done = await copier.copy(secret, e.currentTarget as Element);
		if (!done && shown) getSelection()?.selectAllChildren(shown);
	}
</script>

<div class="reveal" data-testid="key-reveal">
	<p class="note">Store it now: it won't be shown again. SQLBuild keeps only a hash of it.</p>
	<div class="key">
		<code bind:this={shown}>{secret}</code>
		<Button size="sm" variant="outline" onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button>
	</div>
	{#if failed}<p class="note" role="alert">Couldn't copy it here: it's selected, so copy it with Ctrl+C (⌘C on a Mac).</p>{/if}
</div>

<style>
	.reveal {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.note {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.key {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 12px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--background);
	}
	code {
		min-width: 0;
		flex: 1;
		overflow-wrap: anywhere;
		font-family: var(--font-mono);
		font-size: 12px;
		user-select: all;
	}
</style>
