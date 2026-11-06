<script lang="ts">
	import { page } from '$app/state';
	import TriggerView from './trigger-view.svelte';

	const id = $derived(page.params.id ?? '');
</script>

{#key id}
	<TriggerView {id} />
{/key}
