<script lang="ts">
	import { page } from '$app/state';
	import JobView from './job-view.svelte';

	const name = $derived(page.params.name ?? '');
</script>

{#key name}
	<JobView {name} />
{/key}
