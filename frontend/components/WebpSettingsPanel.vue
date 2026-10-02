<script setup lang="ts">
// Every cwebp option that changes the file it writes, one control per flag. Help text is
// cwebp's own (`cwebp -longhelp`), with the flag it sets.
//
// cwebp's two shorthands, -preset and -z, and its no-option defaults are buttons that set the controls the way they
// set the config on the command line: the Rust core applies them, so their values are
// libwebp's own rather than a copy here.
import { computed, ref } from 'vue'
import type { AlphaFiltering, FilterType, ImageHint, Preset, WebpSettings } from '~/composables/useSettings'
import { invokeCommand } from '~/composables/useTauri'

const model = defineModel<WebpSettings>({ required: true })
const s = computed(() => model.value)

const PRESETS: { label: string, value: Preset }[] = [
	{ label: 'default', value: 'default' },
	{ label: 'photo', value: 'photo' },
	{ label: 'picture', value: 'picture' },
	{ label: 'drawing', value: 'drawing' },
	{ label: 'icon', value: 'icon' },
	{ label: 'text', value: 'text' },
]
const LOSSLESS_LEVELS = Array.from({ length: 10 }, (_, level) => ({ label: `-z ${level}${level === 0 ? ' (fastest)' : level === 9 ? ' (slowest)' : ''}`, value: level }))
const FILTER_TYPES: { label: string, value: FilterType }[] = [{ label: 'Strong', value: 'strong' }, { label: 'Simple', value: 'simple' }]
const ALPHA_FILTERS: { label: string, value: AlphaFiltering }[] = [{ label: 'None', value: 'off' }, { label: 'Fast', value: 'fast' }, { label: 'Best', value: 'best' }]
const HINTS: { label: string, value: ImageHint }[] = [{ label: 'None', value: 'default' }, { label: 'Photo', value: 'photo' }, { label: 'Picture', value: 'picture' }, { label: 'Graph', value: 'graph' }]
const TARGETS = [{ label: 'Quality', value: 'none' }, { label: 'File size', value: 'size' }, { label: 'PSNR', value: 'psnr' }]

const preset = ref<Preset>('photo')
const losslessLevel = ref(6)
const presetError = ref('')

async function applyPreset() {
	presetError.value = ''
	try {
		model.value = await invokeCommand<WebpSettings>('webp_apply_preset', { webp: model.value, preset: preset.value })
	}
	catch (error) {
		presetError.value = String(error)
	}
}

async function startFromCwebpDefaults() {
	presetError.value = ''
	try {
		model.value = await invokeCommand<WebpSettings>('webp_cwebp_defaults', { webp: model.value })
	}
	catch (error) {
		presetError.value = String(error)
	}
}

async function applyLosslessLevel() {
	presetError.value = ''
	try {
		model.value = await invokeCommand<WebpSettings>('webp_apply_lossless_level', { webp: model.value, level: losslessLevel.value })
	}
	catch (error) {
		presetError.value = String(error)
	}
}

// -size and -psnr are one choice: libwebp gives the size precedence when both are set.
const targetKind = computed({
	get: () => s.value.target?.kind ?? 'none',
	set: (kind: string) => {
		s.value.target = kind === 'size' ? { kind: 'size', value: 51200 } : kind === 'psnr' ? { kind: 'psnr', value: 42 } : null
	},
})
const alphaCompression = computed({
	get: () => (s.value.alphaCompression ? '1' : '0'),
	set: (value: string) => { s.value.alphaCompression = value === '1' },
})
</script>

<template>
	<div class="flex flex-col gap-8">
		<ControlPanel title="Compression" class="pt-8">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.lossless" label="Lossless" help="-lossless · encode image losslessly" />
				<ControlSlider v-model="s.quality" :label="s.lossless ? 'Compression effort' : 'Quality'" :min="0" :max="100" :decimals="3" :help="s.lossless ? '-q · 0 is fastest and largest, 100 slowest and smallest; not fidelity' : '-q · quality factor (0:small..100:big)'" />
				<ControlSlider v-model="s.method" label="Compression method" :min="0" :max="6" help="-m · compression method (0=fast, 6=slowest)" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="targetKind" label="Aim for" :items="TARGETS" help="-size / -psnr · a target overrides the quality" />
				<ControlNumber v-if="s.target?.kind === 'size'" v-model="s.target.value" label="Target size" :min="1" unit="bytes" help="-size · target size (in bytes)" />
				<ControlNumber v-if="s.target?.kind === 'psnr'" v-model="s.target.value" label="Target PSNR" :min="0.001" :max="10000" :decimals="3" unit="dB" help="-psnr · target PSNR (in dB, typically 42)" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
					<span class="font-semibold text-paleday-fg">libwebp preset</span>
					<div class="flex items-center gap-2">
						<USelect v-model="preset" :items="PRESETS" variant="soft" size="sm" aria-label="libwebp preset" class="min-w-0 flex-1" :ui="{ base: 'bg-paleday-field text-paleday-bright' }" />
						<UButton size="sm" color="neutral" variant="soft" @click="applyPreset">
							Apply
						</UButton>
					</div>
					<p class="text-paleday-dim">
						-preset · resets every encoder control to the preset, keeping the quality
					</p>
				</div>
				<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
					<span class="font-semibold text-paleday-fg">Lossless level</span>
					<div class="flex items-center gap-2">
						<USelect v-model="losslessLevel" :items="LOSSLESS_LEVELS" variant="soft" size="sm" aria-label="Lossless level" class="min-w-0 flex-1" :ui="{ base: 'bg-paleday-field text-paleday-bright' }" />
						<UButton size="sm" color="neutral" variant="soft" @click="applyLosslessLevel">
							Apply
						</UButton>
					</div>
					<p class="text-paleday-dim">
						-z · lossless, with the method and effort of the level
					</p>
				</div>
				<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
					<span class="font-semibold text-paleday-fg">cwebp's defaults</span>
					<div class="flex items-center gap-2">
						<UButton size="sm" color="neutral" variant="soft" @click="startFromCwebpDefaults">
							Start from cwebp's defaults
						</UButton>
					</div>
					<p class="text-paleday-dim">
						no options · every control as a plain cwebp sets it, so the file matches cwebp with no flags
					</p>
				</div>
			</div>
			<p v-if="presetError" class="px-2.5 text-xs text-paleday-error" role="alert">
				{{ presetError }}
			</p>
		</ControlPanel>

		<ControlPanel v-if="s.lossless" title="Lossless">
			<div class="grid grid-cols-3 gap-4">
				<ControlSlider v-model="s.nearLossless" label="Near-lossless" :min="0" :max="100" help="-near_lossless · use near-lossless image preprocessing (0..100=off)" />
				<ControlChoice v-model="s.imageHint" label="Image hint" :items="HINTS" help="-hint · specify image characteristics hint" />
			</div>
		</ControlPanel>

		<ControlPanel title="Transparency">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle :model-value="!s.keepAlpha" label="Discard transparency" help="-noalpha · discard any transparency information" @update:model-value="(discard: boolean) => { s.keepAlpha = !discard }" />
				<ControlToggle v-model="s.exact" label="Exact" help="-exact · preserve RGB values in transparent area" />
				<ControlOptional v-model="s.blendAlpha" label="Blend onto a background" :fallback="0xffffff" help="-blend_alpha · blend colors against background color" unset="-blend_alpha · not given: transparency is kept">
					<ControlColor v-model="s.blendAlpha" label="Background" />
				</ControlOptional>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlSlider v-model="s.alphaQuality" label="Alpha quality" :min="0" :max="100" help="-alpha_q · transparency-compression quality (0..100)" />
				<ControlChoice v-model="alphaCompression" label="Alpha compression" :items="[{ label: 'Lossless', value: '1' }, { label: 'None', value: '0' }]" help="-alpha_method · transparency-compression method (0..1)" />
				<ControlChoice v-model="s.alphaFiltering" label="Alpha filter" :items="ALPHA_FILTERS" help="-alpha_filter · predictive filtering for alpha plane" />
			</div>
		</ControlPanel>

		<ControlDisclosure title="Lossy tuning" :open="!s.lossless">
			<div class="grid grid-cols-4 gap-3">
				<ControlSlider v-model="s.sns" label="Spatial noise shaping" :min="0" :max="100" help="-sns · spatial noise shaping (0:off, 100:max)" />
				<ControlSlider v-model="s.segments" label="Segments" :min="1" :max="4" help="-segments · number of segments to use (1..4)" />
				<ControlSlider v-model="s.passes" label="Analysis passes" :min="1" :max="10" help="-pass · analysis pass number (1..10)" />
				<ControlSlider v-model="s.partitionLimit" label="Partition limit" :min="0" :max="100" help="-partition_limit · limit quality to fit the 512k limit on the first partition (0=no degradation ... 100=full)" />
			</div>
			<div class="grid grid-cols-4 gap-3">
				<ControlChoice v-model="s.filterType" label="Filter type" :items="FILTER_TYPES" help="-strong / -nostrong · strong or simple deblocking filter" />
				<ControlToggle v-model="s.autofilter" label="Auto filter" help="-af · auto-adjust filter strength" />
				<ControlSlider v-model="s.filterStrength" label="Filter strength" :min="0" :max="100" help="-f · filter strength (0=off..100); -af adjusts it per segment" />
				<ControlSlider v-model="s.filterSharpness" label="Filter sharpness" :min="0" :max="7" help="-sharpness · filter sharpness (0:most .. 7:least sharp)" />
			</div>
			<div class="grid grid-cols-4 gap-3">
				<ControlSlider v-model="s.qmin" label="Minimum quality" :min="0" :max="100" help="-qrange · the permissible quality range, low end" />
				<ControlSlider v-model="s.qmax" label="Maximum quality" :min="0" :max="100" help="-qrange · the permissible quality range, high end" />
				<ControlSlider v-model="s.preprocessing" label="Pre-processing" :min="0" :max="7" help="-pre · pre-processing filter: 1 smooths segments, 2 dithers" />
			</div>
			<div class="grid grid-cols-4 gap-3">
				<ControlToggle v-model="s.sharpYuv" label="Sharp YUV" help="-sharp_yuv · use sharper (and slower) RGB->YUV conversion" />
				<ControlToggle v-model="s.jpegLike" label="JPEG-like" help="-jpeg_like · roughly match expected JPEG size" />
			</div>
		</ControlDisclosure>

		<ControlPanel title="Metadata">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.metadata.exif" label="Copy Exif" help="-metadata exif · copy Exif from the input if present" />
				<ControlToggle v-model="s.metadata.icc" label="Copy ICC profile" help="-metadata icc · copy the colour profile from the input if present" />
				<ControlToggle v-model="s.metadata.xmp" label="Copy XMP" help="-metadata xmp · copy XMP from the input if present" />
			</div>
		</ControlPanel>

		<ControlDisclosure title="Animation">
			<p class="px-2.5 text-xs text-paleday-dim">
				For an animated WebP or a GIF only: img2webp's and gif2webp's own options. Still images ignore them.
			</p>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.animation.minimizeSize" label="Minimise size" help="-min_size · search harder for the smallest file; slower, and places no keyframes" />
				<ControlToggle v-model="s.animation.allowMixed" label="Mixed lossy and lossless" help="-mixed · each frame lossy or lossless, whichever is smaller" />
				<ControlToggle v-model="s.animation.loopCompatibility" label="GIF loop compatibility" help="-loop_compatibility · gif2webp: read the GIF's loop count as Chrome up to M62 did" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.animation.kmin" label="Minimum keyframe distance" :fallback="3" unset="-kmin · not given: the tool's default (gif2webp 9 lossless, 3 lossy)">
					<ControlNumber v-model="s.animation.kmin" label="At least, in frames" :min="-2147483648" :max="2147483647" help="-kmin · min distance between key frames" />
				</ControlOptional>
				<ControlOptional v-model="s.animation.kmax" label="Maximum keyframe distance" :fallback="5" unset="-kmax · not given: the tool's default (gif2webp 17 lossless, 5 lossy)">
					<ControlNumber v-model="s.animation.kmax" label="At most, in frames" :min="-2147483648" :max="2147483647" help="-kmax · max distance between key frames; 1 makes every frame a keyframe, 0 none" />
				</ControlOptional>
				<ControlOptional v-model="s.animation.loopCount" label="Set the loop count" :fallback="0" unset="-loop · not given: the source's own loop count is kept">
					<ControlNumber v-model="s.animation.loopCount" label="Plays" :min="0" :max="65535" help="-loop · how many times it plays; 0 is forever" />
				</ControlOptional>
			</div>
		</ControlDisclosure>

		<ControlPanel title="Performance">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.multiThreading" label="Multi-threading" help="-mt · use multi-threading if available" />
				<ControlToggle v-model="s.lowMemory" label="Low memory" help="-low_memory · reduce memory usage (slower encoding)" />
			</div>
		</ControlPanel>
	</div>
</template>
