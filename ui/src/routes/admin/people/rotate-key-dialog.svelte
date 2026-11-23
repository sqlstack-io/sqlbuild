<script lang="ts">
	import type { ServiceAccount } from '$lib/api';
	import MenuSelect from '$lib/components/platform/menu-select.svelte';
	import Modal from '$lib/components/platform/modal.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { formatStamp } from '$lib/format/time';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import KeyReveal from './key-reveal.svelte';
	import type { OverlapChoice } from './types';
	import { OVERLAP_CHOICES, overlapMinutes } from './utils';

	// Rotate in a dialog: choose when the old key stops, then the new key is shown here once.
	type Props = { account: ServiceAccount; open: boolean; onrotated: () => void; onclose: () => void };
	let { account, open, onrotated, onclose }: Props = $props();

	let choice = $state<OverlapChoice>('60');
	let busy = $state(false);
	let error = $state<string | null>(null);
	let secret = $state<string | null>(null);
	const minutes = $derived(overlapMinutes(choice));

	async function rotate() {
		busy = true;
		error = null;
		try {
			secret = (await app.client.rotateServiceAccountKey(account.name, { overlapMinutes: minutes })).secret;
			onrotated();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	function close() {
		secret = null;
		error = null;
		choice = '60';
		onclose();
	}
</script>

<Modal {open} title={secret ? `New key for ${account.name}` : `Rotate ${account.name}'s key`} onclose={close} testid="rotate-dialog">
	{#if secret}
		<KeyReveal {secret} />
	{:else}
		<div class="field">
			<span class="label">Stop the old key</span>
			<MenuSelect value={choice} options={OVERLAP_CHOICES} onchange={(v) => (choice = v)} label="When the old key stops" block testid="overlap-select" />
		</div>
		<p class="note">
			{#if minutes}
				Both keys work until {formatStamp(new Date(app.now + minutes * 60_000).toISOString())}, so whatever uses this account can switch to the new key first.{#if account.keys.length > 1}{' '}The key already being replaced stops now.{/if}
			{:else}
				Anything still using the current key is refused from now on. Choose this if the key may have leaked.
			{/if}
		</p>
		{#if error}<p class="err" role="alert">{error}</p>{/if}
	{/if}
	{#snippet footer()}
		{#if secret}
			<Button size="sm" onclick={close}>Done</Button>
		{:else}
			<Button size="sm" variant="ghost" onclick={close}>Cancel</Button>
			<Button size="sm" disabled={busy} onclick={rotate}>{busy ? 'Rotating…' : 'Rotate key'}</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.label {
		font-size: 12.5px;
	}
	.note {
		margin: 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
		line-height: 1.5;
	}
	.err {
		margin: 0;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
