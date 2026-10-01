<script setup lang="ts">
// The crop and resize every output format shares, in cwebp's order: crop the source, then
// resize what is left. -resize_mode decides whether the resize applies to this image. And how
// a TIFF's alpha is read, which also applies to every format.
import { computed } from 'vue'
import type { EncodeJob, ResizeMode } from '~/composables/useSettings'

const model = defineModel<EncodeJob>({ required: true })
const job = computed(() => model.value)

const MODES: { label: string, value: ResizeMode }[] = [{ label: 'Always', value: 'always' }, { label: 'Only shrink', value: 'downOnly' }, { label: 'Only enlarge', value: 'upOnly' }]
</script>

<template>
	<div class="flex flex-col gap-8">
		<ControlPanel title="Crop">
			<ControlOptional v-model="job.crop" label="Crop the source" :fallback="{ x: 0, y: 0, width: 1024, height: 1024 }" unset="-crop · not given: the whole image">
				<template #default="{ value }">
					<div class="grid grid-cols-4 gap-2">
						<ControlNumber v-model="value.x" label="Left" :min="0" unit="px" help="-crop x" />
						<ControlNumber v-model="value.y" label="Top" :min="0" unit="px" help="-crop y" />
						<ControlNumber v-model="value.width" label="Width" :min="1" unit="px" help="-crop w" />
						<ControlNumber v-model="value.height" label="Height" :min="1" unit="px" help="-crop h" />
					</div>
				</template>
			</ControlOptional>
		</ControlPanel>

		<ControlPanel title="Resize">
			<div class="grid grid-cols-3 gap-3">
				<ControlNumber v-model="job.resize.width" label="Width" :min="0" unit="px" help="-resize w · 0 derives it from the height, keeping the aspect ratio" />
				<ControlNumber v-model="job.resize.height" label="Height" :min="0" unit="px" help="-resize h · 0 derives it from the width; both 0 is no resize" />
				<ControlChoice v-model="job.resize.mode" label="When" :items="MODES" help="-resize_mode · always, down_only or up_only" />
			</div>
		</ControlPanel>

		<ControlPanel title="TIFF input">
			<ControlToggle v-model="job.tiffAlphaLikeReference" label="Read transparency as the official tool does" help="Off: a TIFF's semi-transparent colours are read correctly. On: as cwebp reads them for WebP (straight alpha comes out darker) and heif-enc for HEIC (premultiplied alpha comes out darker), byte for byte. AVIF and JPEG XL are unaffected." />
		</ControlPanel>
	</div>
</template>
