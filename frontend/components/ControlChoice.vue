<script setup lang="ts">
// A labelled set of mutually exclusive choices: the Nanna radio group under a heading.
const model = defineModel<string>({ required: true })

withDefaults(defineProps<{
	label: string
	help?: string
	items: { label: string, value: string, description?: string }[]
	orientation?: 'horizontal' | 'vertical'
	layout?: 'cards' | 'compact'
	disabled?: boolean
}>(), { orientation: 'horizontal', layout: 'compact' })
</script>

<template>
	<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px]" :class="disabled && 'opacity-50'">
		<span class="text-xs font-semibold text-paleday-fg">{{ label }}</span>
		<ChoiceGroup v-model="model" :items="items" :orientation="orientation" :layout="layout" :aria-label="label" :disabled="disabled" />
		<p v-if="help" class="text-xs text-paleday-dim">
			{{ help }}
		</p>
	</div>
</template>
