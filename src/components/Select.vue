<template>
        <button :class="['flex flex-col bg-gray-400 cursor-pointer transition-all duration-300 ease-in-out focus:outline-none', {'rounded-top-20px': isOpen}, {'rounded-20px': !isOpen}, {'hidden': !visible}]" @click="isOpen = !isOpen" @focus="focused = true" @blur="focused = false">
                <div class="flex flex-row w-full h-10 items-center justify-center pointer-events-none">
                        <label class="text-xs mx-4 uppercase flex-1 text-left">{{ label }}</label>
                        <div class="flex justify-center items-center text-gray-800 mr-4">
                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" :class="['fill-current h-3', {'transform-rotate-180': isOpen}, {'animate-bounce': focused }]">
                                        <path fill="#1a202c" d="M6 12L0 0h12z"/>
                                </svg>
                        </div>
                </div>
        <div v-for="item in options" :key="item.value" :class="['flex justify-center items-center transition-size duration-300 ease-in-out overflow-hidden w-full',{'flex h-10 border-gray-800 border-t justify-center items-center uppercase hover:bg-gray-800': isOpen}, {'h-0px w-0px': !isOpen}, {'bg-gray-400': item.value == ''}]" @click="optionClick(item)">
                        <p :class="['flex items-center justify-center text-left mx-2 text-xs transition-size duration-300, ease-in-out', {'w-0px h-0px': !isOpen}, {'h-10': isOpen}]">
                                {{ item.label }}
                        </p>
                </div>
        </button>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import store from '../store';
import { Component } from '../interface/store/Component';

export default defineComponent({ 
        name: 'Select',

        props: {
                category: {
                        type: String,
                        required: true,
                },
                group: {
                        type: String,
                        required: true
                },
                dependency: String,
        },

        data() {
                return {
                        isOpen: false,
                        label: '',
                        options: [{
                                group: '',
                                label: '',
                                value: '',
                        }],
                        dataValue: '',
                        visible: false,
                        focused: false,
                }
        },

        created() {
                // get options from store
                if(this.group && this.category) {
                        this.options = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()];
                        this.label = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()][0].label
                }

                //register compnent with store
                this.register();
        },

        computed: {
                advancedOptions: () => store.getters.advancedOptions,
                selects: () => store.getters.componentsSelect as Component[],
                value(): Component['value'] { return !this.visible ? "undefined" : this.dataValue }
        },

        methods: {
                optionClick: function(option: { group: string; label: string; value: string }) {
                        this.label = option.label;
                        this.dataValue = option.value;
                },

                register: function() {
                        const payload: Component =
                                {
                                        type: 'select',
                                        category: this.category,
                                        group: this.group,
                                        dependency: this.dependency,
                                        label: this.label,
                                        value: this.value,
                                        isOpen: this.isOpen,
                                        visible: this.visible
                                }

                        store.commit('registerComponent', payload);
                },
        },

        watch: {
                advancedOptions: {
                        deep: true,
                        handler() {
                                if(this.group && this.category) {
                                        this.options = this.advancedOptions[this.category.toLowerCase()][this.group.toLowerCase()];
                                        for(const component of this.selects) component.group === this.group ? this.visible = component.visible : false;
                                }
                        }
                },
                category: function() { this.register() },
                group: function() { this.register() },
                dependency: function() { this.register() },
                isOpen: function() { this.register() },
                label: function() { this.register() },
                dataValue: function() { this.register() },
                visible: function() {
                        this.label = this.options[0].label;
                        this.dataValue = this.options[0].value;
                        if(!this.visible) this.register();
                },
        }

});
</script>

<style scoped>
        .rounded-20px {
                border-radius: 20px;
        }

        .rounded-top-20px {
                border-top-right-radius: 20px;
                border-top-left-radius: 20px;
        }

        .transform-rotate-180 {
                transform: rotate(180deg);
        }

        .h-0px {
                height: 0px;
        }

        .w-0px {
                width: 0px;
        }

        .animate-bounce {
                -webkit-animation: shake-vertical 1s ease-in-out both;
                animation: shake-vertical 1s ease-in-out both;
        }

        @-webkit-keyframes shake-vertical {
                0%, 100% {
                        -webkit-transform: translateY(0);
                        transform: translateY(0);
                }
                10%, 30%, 50%, 70% {
                        -webkit-transform: translateY(-8px);
                        transform: translateY(-8px);
                }
                20%, 40%, 60% {
                        -webkit-transform: translateY(8px);
                        transform: translateY(8px);
                }
                80% {
                        -webkit-transform: translateY(6.4px);
                        transform: translateY(6.4px);
                }
                90% {
                        -webkit-transform: translateY(-6.4px);
                        transform: translateY(-6.4px);
                }
        }

        @keyframes shake-vertical {
                0%, 100% {
                        -webkit-transform: translateY(0);
                        transform: translateY(0);
                }
                10%, 30%, 50%, 70% {
                        -webkit-transform: translateY(-8px);
                        transform: translateY(-8px);
                }
                20%, 40%, 60% {
                        -webkit-transform: translateY(8px);
                        transform: translateY(8px);
                }
                80% {
                        -webkit-transform: translateY(6.4px);
                        transform: translateY(6.4px);
                }
                90% {
                        -webkit-transform: translateY(-6.4px);
                        transform: translateY(-6.4px);
                }
        }
</style>