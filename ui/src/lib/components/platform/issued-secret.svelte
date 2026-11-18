<script lang="ts">
	import { copyText } from '$lib/browser/clipboard';
	import Button from '$lib/components/ui/button/button.svelte';
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';

	type Props = { label: string; secret: string; ondismiss: () => void };
	let { label, secret, ondismiss }: Props = $props();

	let copied = $state(false);

	async function copy() {
		copied = await copyText(secret);
	}
</script>

<div class="issued" role="status">
	<KeyRoundIcon size={15} />
	<div class="min-w-0 flex-1">
		<div class="title">{label}</div>
		<p>Copy it now: it's shown only this once and can't be retrieved later. If it's lost, rotate the token.</p>
		<code>{secret}</code>
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
	.actions {
		display: flex;
		gap: 6px;
	}
</style>
