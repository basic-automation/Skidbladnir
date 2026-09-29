<script setup lang="ts">
// Where a piece of metadata comes from: the input as its reference tool reads it, nowhere
// (--ignore-exif, -x strip=exif), or a file (--exif FILE, -x exif=FILE).
import { computed } from 'vue'
import type { MetadataSource } from '~/composables/useSettings'

const model = defineModel<MetadataSource>({ required: true })

defineProps<{ label: string, help?: string }>()

const ITEMS = [
	{ label: 'From the input', value: 'keep' },
	{ label: 'Leave out', value: 'strip' },
	{ label: 'From a file', value: 'file' },
]

const kind = computed({
	get: () => model.value.kind,
	set: (next: string) => {
		model.value = next === 'file' ? { kind: 'file', path: model.value.kind === 'file' ? model.value.path : '' } : { kind: next as 'keep' | 'strip' }
	},
})
const path = computed({
	get: () => (model.value.kind === 'file' ? model.value.path : ''),
	set: (next: string) => { model.value = { kind: 'file', path: next } },
})
</script>

<template>
	<div class="flex min-w-0 flex-col">
		<ControlChoice v-model="kind" :label="label" :help="help" :items="ITEMS" />
		<ControlFile v-if="model.kind === 'file'" v-model="path" :label="`${label} file`" class="pl-5" />
	</div>
</template>
