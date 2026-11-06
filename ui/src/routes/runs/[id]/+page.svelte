<script lang="ts">
	import { page } from '$app/state';
	import RunView from './run-view.svelte';

	const runId = $derived(page.params.id ?? '');
</script>

{#key runId}
	<RunView {runId} />
{/key}
