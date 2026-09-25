<script setup lang="ts">
const model = defineModel<boolean>({ required: true })

defineProps<{ label: string, help?: string, disabled?: boolean }>()
</script>

<template>
	<UCheckbox
		v-model="model"
		:disabled="disabled"
		:label="label"
		:description="help"
		class="text-left"
	/>
</template>
