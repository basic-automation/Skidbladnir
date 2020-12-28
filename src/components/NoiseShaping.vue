<template>
        <div :class="['flex flex-col flex-1 justify-center items-center transition-all ease-in-out duration-500 w-full', { 'hidden': !visible }]">
                <h1 class="flex items-center text-gray-400 uppercase text-lg antialiased">Noise Shaping</h1>
                <div class="grid grid-cols-1 sm:grid-cols-2 items-start justify-center gap-4 mt-4 max-w-4xl">
                        <Slider group="amplitude" category="noiseShaping" dependency="noiseShaping" />
                        <Slider group="numberOfSegments" category="noiseShaping" dependency="noiseShaping" :rtl="true" />
                </div>
        </div>
</template>

<script lang="ts">
        import { AdvancedOptions } from '@/interface/store/AdvancedOptions';
        import { Component } from '@/interface/store/Component';
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
                        advancedOptions: () => store.getters.advancedOptions as AdvancedOptions,
                        categories: () => store.getters.componentCategory as Component[],
                },

                watch: {
                        advancedOptions: {
                                deep: true,
                                handler() { if(this.group && this.category) for(const component of this.categories) component.group === this.group ? this.visible = component.visible : false; }
                        },
                }
        })
</script>