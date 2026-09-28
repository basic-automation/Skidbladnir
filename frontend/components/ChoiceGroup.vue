<script setup lang="ts">
// Nuxt UI's radio group in the Nanna skin: no outline and no dot. The chosen item carries
// an accent bar on its left edge and an accent label; the rest are bare text on a rounded
// hover area. `variant="card"` is used for its anatomy — the whole item is the <label>, so
// the text is both the click target and the radio's accessible name.
import { computed } from 'vue'

const model = defineModel<string>()

const props = withDefaults(defineProps<{
	items: { label: string, value: string, description?: string }[]
	orientation?: 'horizontal' | 'vertical'
	/** `cards` spreads described options across the row; `compact` is a run of short words. */
	layout?: 'cards' | 'compact'
}>(), { orientation: 'horizontal', layout: 'compact' })

const ui = computed(() => ({
	fieldset: props.layout === 'cards' ? 'gap-4 flex-nowrap' : props.orientation === 'vertical' ? 'gap-0.5' : 'gap-1',
	item: [
		'group border-0 border-l-4 border-transparent rounded-lg transition-colors cursor-pointer',
		'hover:not-has-disabled:not-has-focus-visible:not-has-data-[state=checked]:border-transparent',
		'hover:not-has-data-[state=checked]:bg-paleday-field/70',
		'has-data-[state=checked]:border-primary has-data-[state=checked]:bg-transparent has-data-[state=checked]:rounded-none',
		props.layout === 'cards' ? 'flex-1 min-w-0 px-2.5 py-[7px]' : 'px-2 py-1',
		props.orientation === 'vertical' ? 'w-full' : '',
	].join(' '),
	wrapper: 'items-start text-left gap-1',
	label: 'text-xs font-semibold text-paleday-fg group-has-data-[state=checked]:text-paleday-accent-text',
	description: 'text-xs font-[450] text-paleday-dim',
}))
</script>

<template>
	<URadioGroup
		v-model="model"
		:items="items"
		:orientation="orientation"
		variant="card"
		indicator="hidden"
		size="sm"
		:ui="ui"
	/>
</template>
