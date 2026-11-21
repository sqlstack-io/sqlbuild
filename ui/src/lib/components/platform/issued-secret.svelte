<script lang="ts">
	import { copyText } from '$lib/browser/clipboard';
	import Button from '$lib/components/ui/button/button.svelte';
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';

	type Props = {
		label: string;
		secret: string;
		ondismiss: () => void;
		/** What to do with it, in place of the location token's wording. */
		note?: string;
		/** Inside a panel that already has its own padding. */
		flush?: boolean;
	};
	let { label, secret, ondismiss, note = "Copy it now: it's shown only this once and can't be retrieved later. If it's lost, rotate the token.", flush = false }: Props = $props();

	let copied = $state(false);
	// Copying can fail, for example where the browser blocks it; the secret is then selected to copy by hand.
	let failed = $state(false);
	let shown = $state<HTMLElement>();

	async function copy() {
		copied = await copyText(secret);
		failed = !copied;
		if (failed && shown) getSelection()?.selectAllChildren(shown);
	}
</script>

<div class="issued" class:flush role="status">
	<KeyRoundIcon size={15} />
	<div class="min-w-0 flex-1">
		<div class="title">{label}</div>
		<p>{note}</p>
		<code bind:this={shown}>{secret}</code>
		{#if failed}<p class="manual" role="alert">Couldn't copy it here: it's selected, so copy it with Ctrl+C (⌘C on a Mac).</p>{/if}
	</div>
	<div class="actions">
		<Button size="sm" onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button>
		<Button size="sm" onclick={ondismiss}>Done</Button>
	</div>
</div>

<style>
	.issued {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		margin: 12px 20px 0;
		padding: 10px 12px;
		border: 1px solid color-mix(in srgb, var(--sb-success) 40%, var(--border));
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-success) 7%, var(--background));
		font-size: 12.5px;
	}
	.issued.flush {
		margin: 0;
	}
	.issued > :global(svg) {
		margin-top: 2px;
		color: var(--sb-success);
	}
	.title {
		font-weight: 600;
	}
	p {
		margin: 2px 0 6px;
		color: var(--muted-foreground);
	}
	code {
		display: block;
		overflow-wrap: anywhere;
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.manual {
		margin: 6px 0 0;
		color: var(--sb-warning);
	}
	.actions {
		display: flex;
		gap: 6px;
	}
</style>
