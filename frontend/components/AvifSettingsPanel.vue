<script setup lang="ts">
// Every avifenc option that changes the file it writes for a still image, with libaom.
// Help text is avifenc's own (`avifenc --help`), with the flag it sets. An option avifenc
// treats as "not given" has a switch, because giving it any value changes more than that
// value: an unset quality is what --target-size searches.
import { computed } from 'vue'
import type { AvifSettings, CleanAperture, Tiling, YuvFormat } from '~/composables/useSettings'

const model = defineModel<AvifSettings>({ required: true })
const s = computed(() => model.value)

const DEPTHS = [{ label: 'Auto', value: 'auto' }, { label: '8', value: '8' }, { label: '10', value: '10' }, { label: '12', value: '12' }]
const EXTENSIONS = [{ label: 'None', value: 'none' }, { label: '4', value: '4' }, { label: '8', value: '8' }]
const YUV: { label: string, value: YuvFormat }[] = [{ label: 'Auto', value: 'auto' }, { label: '4:4:4', value: 'yuv444' }, { label: '4:2:2', value: 'yuv422' }, { label: '4:2:0', value: 'yuv420' }, { label: '4:0:0', value: 'yuv400' }]
const RANGES = [{ label: 'Full', value: 'full' }, { label: 'Limited', value: 'limited' }]
const APERTURES = [{ label: 'None', value: 'none' }, { label: 'Crop rectangle', value: 'crop' }, { label: 'Fractions', value: 'raw' }]

const depth = computed({
	get: () => (s.value.depth === null ? 'auto' : String(s.value.depth)),
	set: (value: string) => {
		s.value.depth = value === 'auto' ? null : Number(value)
		if (s.value.depth === null) s.value.depthExtension = null
	},
})
const extension = computed({
	get: () => (s.value.depthExtension === null ? 'none' : String(s.value.depthExtension)),
	set: (value: string) => { s.value.depthExtension = value === 'none' ? null : Number(value) },
})
const range = computed({
	get: () => (s.value.limitedRange ? 'limited' : 'full'),
	set: (value: string) => { s.value.limitedRange = value === 'limited' },
})
const tiling = computed({
	get: () => s.value.tiling.kind,
	set: (kind: string) => { s.value.tiling = (kind === 'manual' ? { kind: 'manual', rowsLog2: 0, colsLog2: 0 } : { kind: 'automatic' }) as Tiling },
})
const aperture = computed({
	get: () => s.value.cleanAperture?.kind ?? 'none',
	set: (kind: string) => {
		s.value.cleanAperture = (kind === 'crop' ? { kind: 'crop', values: [0, 0, 1, 1] } : kind === 'raw' ? { kind: 'raw', values: [1, 1, 1, 1, 0, 1, 0, 1] } : null) as CleanAperture | null
	},
})
const allJobs = computed({
	get: () => s.value.jobs === null,
	set: (all: boolean) => { s.value.jobs = all ? null : 1 },
})
const CROP_LABELS = ['X', 'Y', 'Width', 'Height']
const CLAP_LABELS = ['Width numerator', 'Width denominator', 'Height numerator', 'Height denominator', 'Horizontal offset numerator', 'Horizontal offset denominator', 'Vertical offset numerator', 'Vertical offset denominator']

function addCodecOption() {
	s.value.codecOptions.push({ key: '', value: '' })
}
</script>

<template>
	<div class="flex flex-col gap-8">
		<ControlPanel title="Quality" class="pt-8">
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.quality" label="Colour quality" :fallback="60" unset="-q · not given: avifenc's 60, or searched by --target-size">
					<ControlSlider v-model="s.quality" label="Quality" :min="0" :max="100" help="-q · quality for color in 0..100 where 100 is lossless" />
				</ControlOptional>
				<ControlOptional v-model="s.qualityAlpha" label="Alpha quality" :fallback="60" unset="--qalpha · not given: follows the colour quality">
					<ControlSlider v-model="s.qualityAlpha" label="Alpha quality" :min="0" :max="100" help="--qalpha · quality for alpha in 0..100 where 100 is lossless" />
				</ControlOptional>
				<ControlOptional v-model="s.speed" label="Speed" :fallback="6" unset="-s default · libaom's own default">
					<ControlSlider v-model="s.speed" label="Speed" :min="0" :max="10" help="-s · encoder speed in 0..10 where 0 is the slowest, 10 is the fastest" />
				</ControlOptional>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.lossless" label="Lossless" help="-l · set all defaults to encode losslessly" />
				<ControlOptional v-model="s.targetSize" label="Target file size" :fallback="51200" unset="--target-size · not given">
					<ControlNumber v-model="s.targetSize" label="Target size" :min="1" unit="bytes" help="--target-size · set target file size in bytes (up to 7 times slower)" />
				</ControlOptional>
			</div>
		</ControlPanel>

		<ControlPanel title="Pixel format">
			<div class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="depth" label="Bit depth" :items="DEPTHS" help="-d · output bit depth per channel; auto is 8 for 8-bit input, 12 for deeper" />
				<ControlChoice v-if="s.depth === 8 || s.depth === 12" v-model="extension" label="Depth extension" :items="EXTENSIONS" help="-d D,E · a hidden extension image reaching 16 bits (8,8 and 12,4 and 12,8)" />
				<ControlChoice v-model="s.yuv" label="YUV format" :items="YUV" help="-y · auto honours a JPEG's own format; 4:0:0 for grayscale PNG; otherwise 4:4:4" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="range" label="YUV range" :items="RANGES" help="-r · YUV range" />
				<ControlToggle v-model="s.premultiply" label="Premultiply alpha" help="-p · premultiply color by the alpha channel and signal this in the AVIF" />
				<ControlToggle v-model="s.sharpYuv" label="Sharp YUV" help="--sharpyuv · sharp RGB to YUV420 conversion; only for 4:2:0 output" />
			</div>
		</ControlPanel>

		<ControlPanel title="Colour and metadata">
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.cicp" label="Colour signalling (CICP)" :fallback="{ primaries: 1, transfer: 13, matrix: 6 }" help="--cicp P/T/M · set CICP values (nclx colr box); 2 leaves one unspecified" unset="--cicp · not given: from the input">
					<template #default="{ value }">
						<ControlNumber v-model="value.primaries" label="Colour primaries" :min="0" :max="255" help="P, e.g. 1 BT.709, 9 BT.2020, 12 Display P3" />
						<ControlNumber v-model="value.transfer" label="Transfer characteristics" :min="0" :max="255" help="T, e.g. 13 sRGB, 16 PQ, 18 HLG" />
						<ControlNumber v-model="value.matrix" label="Matrix coefficients" :min="0" :max="255" help="M, e.g. 6 BT.601, 1 BT.709, 0 identity" />
					</template>
				</ControlOptional>
				<ControlOptional v-model="s.clli" label="Content light level" :fallback="[1000, 400]" unset="--clli · not given">
					<template #default="{ value }">
						<ControlNumber v-model="value[0]" label="MaxCLL" :min="0" :max="65535" unit="cd/m²" help="--clli · maximum content light level" />
						<ControlNumber v-model="value[1]" label="MaxPALL" :min="0" :max="65535" unit="cd/m²" help="--clli · maximum picture-average light level" />
					</template>
				</ControlOptional>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlMetadata v-model="s.icc" label="ICC profile" help="--ignore-icc / --icc FILE" />
				<ControlMetadata v-model="s.exif" label="Exif" help="--ignore-exif / --exif FILE" />
				<ControlMetadata v-model="s.xmp" label="XMP" help="--ignore-xmp / --xmp FILE" />
			</div>
		</ControlPanel>

		<ControlDisclosure title="Layout">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.progressive" label="Progressive" help="--progressive · a simple layered image supporting progressive rendering" />
				<ControlOptional v-model="s.grid" label="Grid" :fallback="{ columns: 2, rows: 2 }" unset="-g · not given: one image">
					<template #default="{ value }">
						<ControlNumber v-model="value.columns" label="Columns" :min="1" :max="256" help="-g MxN · M columns" />
						<ControlNumber v-model="value.rows" label="Rows" :min="1" :max="256" help="-g MxN · N rows" />
					</template>
				</ControlOptional>
				<ControlOptional v-model="s.scalingMode" label="Scaling mode" :fallback="{ numerator: 1, denominator: 2 }" unset="--scaling-mode · not given: 1/1">
					<template #default="{ value }">
						<ControlNumber v-model="value.numerator" label="Numerator" :min="0" help="--scaling-mode N/D · frame scaling as a fraction" />
						<ControlNumber v-model="value.denominator" label="Denominator" :min="1" />
					</template>
				</ControlOptional>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="tiling" label="Tiling" :items="[{ label: 'Automatic', value: 'automatic' }, { label: 'Manual', value: 'manual' }]" help="--autotiling / --tilerowslog2, --tilecolslog2" />
				<template v-if="s.tiling.kind === 'manual'">
					<ControlSlider v-model="s.tiling.rowsLog2" label="Tile rows (log2)" :min="0" :max="6" help="--tilerowslog2 · log2 of number of tile rows in 0..6" />
					<ControlSlider v-model="s.tiling.colsLog2" label="Tile columns (log2)" :min="0" :max="6" help="--tilecolslog2 · log2 of number of tile columns in 0..6" />
				</template>
			</div>
		</ControlDisclosure>

		<ControlDisclosure title="Quantizers (deprecated in avifenc)">
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.quantizer" label="Colour quantizers" :fallback="{ min: 0, max: 63 }" unset="--min / --max · not given">
					<template #default="{ value }">
						<ControlSlider v-model="value.min" label="Minimum" :min="0" :max="63" help="--min QP" />
						<ControlSlider v-model="value.max" label="Maximum" :min="0" :max="63" help="--max QP" />
					</template>
				</ControlOptional>
				<ControlOptional v-model="s.alphaQuantizer" label="Alpha quantizers" :fallback="{ min: 0, max: 63 }" unset="--minalpha / --maxalpha · not given">
					<template #default="{ value }">
						<ControlSlider v-model="value.min" label="Minimum" :min="0" :max="63" help="--minalpha QP" />
						<ControlSlider v-model="value.max" label="Maximum" :min="0" :max="63" help="--maxalpha QP" />
					</template>
				</ControlOptional>
			</div>
		</ControlDisclosure>

		<ControlDisclosure title="Transform properties">
			<p class="px-2.5 text-xs text-paleday-dim">
				Written into the file for the viewer to apply; the pixels are not changed.
			</p>
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.irot" label="Rotation" :fallback="1" unset="--irot · not given">
					<ControlChoice :model-value="String(s.irot)" label="Anticlockwise" :items="[{ label: '0°', value: '0' }, { label: '90°', value: '1' }, { label: '180°', value: '2' }, { label: '270°', value: '3' }]" help="--irot · (90 * ANGLE) degree rotation anti-clockwise" @update:model-value="(v: string) => { s.irot = Number(v) }" />
				</ControlOptional>
				<ControlOptional v-model="s.imir" label="Mirroring" :fallback="1" unset="--imir · not given">
					<ControlChoice :model-value="String(s.imir)" label="Axis" :items="[{ label: 'Top-to-bottom', value: '0' }, { label: 'Left-to-right', value: '1' }]" help="--imir · 0=top-to-bottom, 1=left-to-right" @update:model-value="(v: string) => { s.imir = Number(v) }" />
				</ControlOptional>
				<ControlOptional v-model="s.pasp" label="Pixel aspect ratio" :fallback="[1, 1]" unset="--pasp · not given">
					<template #default="{ value }">
						<ControlNumber v-model="value[0]" label="Horizontal spacing" :min="0" help="--pasp H,V" />
						<ControlNumber v-model="value[1]" label="Vertical spacing" :min="0" />
					</template>
				</ControlOptional>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="aperture" label="Clean aperture" :items="APERTURES" help="--crop CROPX,CROPY,CROPW,CROPH or --clap WN,WD,HN,HD,HON,HOD,VON,VOD" />
				<div v-if="s.cleanAperture" class="col-span-2 grid grid-cols-4 gap-2">
					<ControlNumber v-for="(_, index) in s.cleanAperture.values" :key="index" v-model="s.cleanAperture.values[index]" :label="(s.cleanAperture.kind === 'crop' ? CROP_LABELS : CLAP_LABELS)[index]!" :min="0" />
				</div>
			</div>
		</ControlDisclosure>

		<ControlDisclosure title="libaom options">
			<p class="px-2.5 text-xs text-paleday-dim">
				-a KEY[=VALUE] · passed straight to libaom, in order. Prefix a key with color: or alpha: to apply it to one plane only, for example tune=ssim, color:aq-mode=1, alpha:end-usage=q.
			</p>
			<div v-for="(option, index) in s.codecOptions" :key="index" class="flex items-end gap-2">
				<ControlText v-model="option.key" :label="`Option ${index + 1} key`" placeholder="tune" class="flex-1" />
				<ControlText v-model="option.value" :label="`Option ${index + 1} value`" placeholder="ssim" class="flex-1" />
				<UButton color="neutral" variant="ghost" icon="i-lucide-x" :aria-label="`Remove option ${index + 1}`" class="mb-2" @click="s.codecOptions.splice(index, 1)" />
			</div>
			<div class="px-2.5">
				<UButton size="sm" color="neutral" variant="soft" icon="i-material-symbols-add" @click="addCodecOption">
					Add an option
				</UButton>
			</div>
		</ControlDisclosure>

		<ControlPanel title="Performance">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="allJobs" label="Every core" help="-j all · number of jobs (worker threads)" />
				<ControlNumber v-if="!allJobs && s.jobs !== null" v-model="s.jobs" label="Worker threads" :min="1" help="-j · number of jobs" />
			</div>
		</ControlPanel>
	</div>
</template>
