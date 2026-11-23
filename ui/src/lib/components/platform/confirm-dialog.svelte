<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import { errorMessage } from '$lib/platform/labels';
	import Modal from './modal.svelte';

	// Asks before an action; a destructive one has the only red button, here. Errors stay in the dialog.
	type Props = {
		open: boolean;
		title: string;
		body: string;
		confirm: string;
		destructive?: boolean;
		onconfirm: () => Promise<unknown>;
		onclose: () => void;
	};
	let { open, title, body, confirm, destructive = false, onconfirm, onclose }: Props = $props();

	let busy = $state(false);
	let error = $state<string | null>(null);
	async function run() {
		busy = true;
		error = null;
		try {
			await onconfirm();
			onclose();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<Modal {open} {title} onclose={() => ((error = null), onclose())} testid="confirm-dialog">
	<p class="body">{body}</p>
	{#if error}<p class="err" role="alert">{error}</p>{/if}
	{#snippet footer()}
		<Button size="sm" variant="ghost" onclick={onclose}>Cancel</Button>
		<Button size="sm" variant={destructive ? 'destructive' : 'default'} disabled={busy} onclick={run}>{busy ? 'Working…' : confirm}</Button>
	{/snippet}
</Modal>

<style>
	.body {
		margin: 0;
		font-size: 13px;
		line-height: 1.55;
	}
	.err {
		margin: 0;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
