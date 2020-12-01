<template>
        <div :class="['flex bg-gray-400 rounded-full h-10 flow-row w-full justify-center items-center', { 'hidden': !visible }]">
                <label :class="['flex text-xs uppercase mx-4 whitespace-no-wrap']">{{ label }}</label>
                <input :class="['flex flex-1 focus:outline-none min-w-0', {'reversed-range': rtl}]" type="range" :min="min" :max="max" v-model.number="inputValue"/>
                <label :class="['flex text-xs uppercase mx-4']"> {{ inputValue }} </label>
        </div>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from '../store';

        export default defineComponent({
                name: 'Slider',

                props: {
                        group: String,
                        category: String,
                        dependency: String,
                        rtl: {
                                type: Boolean,
                                required: false,
                                default: false,
                        },
                },

                data() {
                        return {
                                label: '',
                                inputValue: 1,
                                visible: false,
                                min: 1,
                                max: 1,
                        }
                },

                computed: {
                        advancedOptions: () => store.getters.advancedOptions,
                        sliders: () => store.getters.componentSlider as [{ category: string; group: string; dependency: string; value: number; label: string; min: number; max: number; visible: boolean }],
                },

                methods: {
                        register: function() {
                                const payload =
                                        { 
                                                category: this.category, 
                                                group: this.group,
                                                dependency: this.dependency,
                                                value: this.inputValue,
                                                label: this.label,
                                                min: this.min,
                                                max: this.max,
                                                visible: this.visible,
                                        }

                                store.commit('registerCompnentSliderGroup', payload);
                        },
                },

                created() {
                        if(this.group && this.category) {
                                if(this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()]) {
                                        this.label = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].label;
                                        this.min = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].min;
                                        this.max = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].max;
                                        this.inputValue = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].value;
                                }
                        }

                        this.register();
                },

                watch: {
                        sliders: {
                                deep: true,
                                handler() {
                                        if(this.group && this.category) {
                                                for(let i= 0; i < this.sliders.length; i++) {
                                                        if(typeof this.sliders[i] !== 'undefined' && this.sliders[i].group === this.group){
                                                                this.label = this.sliders[i].label;
                                                                this.inputValue = this.sliders[i].value;
                                                                this.visible = this.sliders[i].visible;
                                                                this.min = this.sliders[i].min;
                                                                this.max = this.sliders[i].max;
                                                        }
                                                }
                                        }
                                }
                        },
                        inputValue: function() {
                                this.register();
                        }
                }
        });
</script>

<style scoped>
        input[type='range'] {
                -webkit-appearance: none;
                background: transparent;
        }

        input[type='range']::-webkit-slider-runnable-track {
                height: 0.05rem;
                cursor: pointer;
                background: rgba(30, 41, 59, 1);
        }

        input[type='range']::-webkit-slider-thumb {
                -webkit-appearance: none;
                height: 1rem;
                width: 1rem;
                background: rgba(30, 41, 59, 1);
                border: 1px solid;
                border-color: rgba(148, 163, 184, 1);
                border-radius: 100%;
                margin-top: -.4rem;
        }

        .reversed-range {
                direction: rtl
        }
</style>
