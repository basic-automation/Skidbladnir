<script setup lang="ts" generic="V extends string | number | boolean | null">
// A labelled drop-down, for choices too many to lay out as a radio group.
const model = defineModel<V>({ required: true })

defineProps<{
	label: string
	help?: string
	items: { label: string, value: V }[]
	disabled?: boolean
}>()
</script>

<template>
	<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
		<span class="font-semibold text-paleday-fg">{{ label }}</span>
		<USelect v-model="model" :items="items" :disabled="disabled" variant="soft" size="sm" :aria-label="label" :ui="{ base: 'bg-paleday-field text-paleday-bright' }" />
		<p v-if="help" class="text-paleday-dim">
			{{ help }}
		</p>
	</div>
</template>
