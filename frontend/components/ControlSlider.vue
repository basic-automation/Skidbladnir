<script setup lang="ts">
// A labelled slider with a live numeric readout — the job the Electron UI's `-info`
// spans did, kept in one place instead of wired up nine times.
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
	<div class="text-left" :class="disabled && 'opacity-40'">
		<div class="flex items-baseline justify-between gap-3">
			<span class="text-sm font-medium text-palenight-bright">{{ label }}</span>
			<output class="font-mono text-sm tabular-nums text-palenight-green" aria-live="off">{{ model }}</output>
		</div>
		<USlider
			v-model="model"
			class="mt-2"
			:min="min"
			:max="max"
			:step="step ?? 1"
			:disabled="disabled"
			size="sm"
			:aria-label="label"
		/>
		<p v-if="help" class="mt-1 text-xs text-palenight-comment">
			{{ help }}
		</p>
	</div>
</template>
