<script setup lang="ts">
// A labelled range control with a live numeric readout, which is what the Electron UI's
// `-info` spans were for.
const model = defineModel<number>({ required: true })

defineProps<{
	label: string
	min: number
	max: number
	step?: number
	help?: string
	disabled?: boolean
}>()
</script>

<template>
	<label class="block" :class="disabled && 'opacity-40'">
		<span class="flex items-baseline justify-between gap-3">
			<span class="text-sm font-medium">{{ label }}</span>
			<output class="font-mono text-sm tabular-nums text-slate-600 dark:text-slate-300">{{ model }}</output>
		</span>
		<input
			v-model.number="model"
			type="range"
			:min="min"
			:max="max"
			:step="step ?? 1"
			:disabled="disabled"
			class="mt-1 w-full accent-sky-600"
		>
		<span v-if="help" class="mt-0.5 block text-xs text-slate-500 dark:text-slate-400">{{ help }}</span>
	</label>
</template>
