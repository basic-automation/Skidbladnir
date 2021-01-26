<template>
        <div :class="['flex bg-gray-400 rounded-full h-10 flow-row w-full justify-center items-center', { 'hidden': !visible }]">
                <label :class="['flex text-xs uppercase mx-4 whitespace-no-wrap']">{{ label }}</label>
                <input :class="['flex flex-1 focus:outline-none min-w-0', {'reversed-range': rtl}]" type="range" :min="min" :max="max" v-model.number="inputValue" @input="register()"/>
                <label :class="['flex text-xs uppercase mx-4']"> {{ inputValue }} </label>
        </div>
</template>

<script lang="ts">
        import { defineComponent } from 'vue';
        import store from '../store';
        import { Component } from '../interface/store/Component';

        export default defineComponent({
                name: 'Slider',

                props: {
                        group: {
                                type: String,
                                required: true
                        },
                        category: {
                                type: String,
                                required: true
                        },
                        dependency: {
                                type: String,
                                required: true,
                        },
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
                        sliders: () => store.getters.componentSlider as Component[],
                        value(): Component['value'] { return !this.visible ? "undefined" : this.inputValue }
                },

                methods: {
                        register: function() {
                                const newComponent: Component = {
                                        type: 'slider',
                                        category: this.category,
                                        group: this.group,
                                        dependency: this.dependency,
                                        label: this.label,
                                        value: this.value,
                                        valueMin: this.min,
                                        valueMax: this.max,
                                        visible: this.visible
                                }

                                //console.log('Registering Component: ', JSON.stringify(newComponent, null, '\t'));
                                store.commit('registerComponent', newComponent);
                        },
                },

                created() {
                        if(this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()]) {
                                this.label = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].label;
                                this.min = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].min;
                                this.max = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].max;
                                this.inputValue = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()].value;
                        }

                        this.register();
                },

                watch: {
                        sliders: {
                                deep: true,
                                handler() {
                                        if(this.group && this.category) {
                                                for(const component of this.sliders) {
                                                        if(typeof component !== 'undefined' && component.group === this.group) {
                                                                this.label = component.label as string;
                                                                component.value === 'undefined' ? this.inputValue = component.valueMax as number : this.inputValue = component.value as number;
                                                                this.visible = component.visible;
                                                                this.min = component.valueMin as number;
                                                                this.max = component.valueMax as number;
                                                        }
                                                }
                                        }
                                }
                        },

                        inputValue: function() {
                                this.register();
                        },

                        visible: function() {
                                if(!this.visible) this.register();
                                else {
                                        for(const component of this.sliders)  if(this.group === component.group && this.value !== component.value) this.register();
                                }
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
