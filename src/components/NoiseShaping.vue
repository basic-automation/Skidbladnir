<template>
        <div :class="['flex flex-col flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full', { 'hidden': !visible }]">
                <h1 class="flex items-center text-gray-500 uppercase text-lg antialiased">Noise Shaping</h1>
                <div class="grid grid-cols-1 sm:grid-cols-2 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Slider group="amplitude" category="noiseShaping" dependency="noiseShaping" />
                        <Slider group="numberOfSegments" category="noiseShaping" dependency="noiseShaping" :rtl="true" />
                </div>
        </div>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'NoiseShaping',

                data() {
                        return {
                                category: 'category',
                                group: 'CATnoiseShaping',
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