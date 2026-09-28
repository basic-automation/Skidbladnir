<script setup lang="ts">
// A settings section in the Nanna layout: a plain heading over its controls, with no
// frame around them. Grouping is carried by spacing alone.
defineProps<{ title: string }>()
</script>

<template>
	<section class="flex w-full flex-col gap-4">
		<h2 class="text-sm font-semibold text-paleday-bright">
			{{ title }}
		</h2>
		<slot />
	</section>
</template>
