<script lang="ts">
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';

	type Props = { label: string; secret: string; ondismiss: () => void };
	let { label, secret, ondismiss }: Props = $props();

	let copied = $state(false);

	async function copy() {
		await navigator.clipboard.writeText(secret);
		copied = true;
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
		<button onclick={copy}>{copied ? 'Copied' : 'Copy'}</button>
		<button onclick={ondismiss}>Done</button>
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
	.actions button {
		height: 28px;
		padding: 0 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		font-size: 12.5px;
	}
	.actions button:hover {
		background: var(--sb-hover);
	}
</style>
