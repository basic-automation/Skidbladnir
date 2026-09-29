<script setup lang="ts">
// cjxl's three-way switches: the flag left out (the encoder decides), =0, or =1.
import type { Tristate } from '~/composables/useSettings'

const model = defineModel<Tristate>({ required: true })

defineProps<{ label: string, help?: string }>()

const ITEMS = [
	{ label: 'Encoder decides', value: 'default' },
	{ label: 'Off', value: 'off' },
	{ label: 'On', value: 'on' },
]
</script>

<template>
	<ControlChoice v-model="model" :label="label" :help="help" :items="ITEMS" />
</template>
