<template>
        <div :class="['flex flex-col flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full', { 'hidden': !visible }]">
                <h1 class="flex items-center text-gray-500 uppercase text-lg antialiased">Compression</h1>
                <div class="grid grid-cols-1 sm:grid-cols-2 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Radio group="compression" id="fileSize" label="File Size" />
                        <Radio group="compression" id="PSNR" label="Peak Signal to Noise Ratio (PSNR)" />
                </div>
                <div class="grid grid-cols-1 sm:grid-cols-2 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Slider group="targetSize" category="compression" dependency="compression" />
                        <Slider group="targetPSNR" category="compression" dependency="compression" />
                        <Slider group="numberOfPasses" category="compression" dependency="compression" />
                </div>
        </div>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'Compression',

                data() {
                        return {
                                category: 'category',
                                group: 'CATcompression',
                                visible: false,
                        }
                },

                computed: {
                        advancedOptions: () => store.getters.advancedOptions,
                        categories: () => store.getters.componentCategory as [{ category: string; group: string; visible: boolean }]
                },

                watch: {
                        advancedOptions: {
                                deep: true,
                                handler() {
                                        if(this.group && this.category) {
                                                for(let i= 0; i < this.categories.length; i++) {
                                                        if(this.categories[i].group === this.group) {
                                                                this.visible = this.categories[i].visible;
                                                        }
                                                }
                                        }
                                }
                        },
                }
        })
</script>