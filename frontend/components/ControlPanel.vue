<script setup lang="ts">
// A titled group of controls, matching the Electron UI's grouping so the layout is
// recognisable to someone moving across.
defineProps<{ title: string }>()
</script>

<template>
	<section class="rounded-lg border border-slate-200 p-4 dark:border-slate-800">
		<h2 class="mb-3 text-xs font-semibold tracking-wide text-slate-500 uppercase dark:text-slate-400">
			{{ title }}
		</h2>
		<div class="flex flex-col gap-4">
			<slot />
		</div>
	</section>
</template>
