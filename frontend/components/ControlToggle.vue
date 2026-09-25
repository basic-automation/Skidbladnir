<script setup lang="ts">
const model = defineModel<boolean>({ required: true })

defineProps<{ label: string, help?: string, disabled?: boolean }>()
</script>

<template>
	<label class="flex gap-2" :class="disabled && 'opacity-40'">
		<input v-model="model" type="checkbox" :disabled="disabled" class="mt-0.5 size-4 shrink-0 accent-sky-600">
		<span>
			<span class="text-sm font-medium">{{ label }}</span>
			<span v-if="help" class="block text-xs text-slate-500 dark:text-slate-400">{{ help }}</span>
		</span>
	</label>
</template>
