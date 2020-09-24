<template>
        <button class="flex h-10 bg-gray-500 rounded-full items-center cursor-pointer flex-shrink-0 focus:outline-none" @click="clicked(); isFocused = false" @focus="isFocused = true" @blur="isFocused = false">
                <label class="flex ml-4 justify-start items-center h-full text-xs cursor-pointer antialiased flex-shrink-0">{{ label }}</label>
                <div class="ba-radio ml-4 mr-2 border-gray-900 cursor-pointer antialiased flex-shrink-0" :class="{ 'ba-radio-checked': isSelected, 'ba-radio-focused': isFocused }"></div>
        </button>
        <input class="hidden" type="radio" :id="id" :name="group" :checked="isSelected" :value="value">
</template>

<script lang="ts">
/* eslint-disable prefer-const */
//import _ from 'lodash';
import { defineComponent } from 'vue';
import store from '../store';

export default defineComponent({
        name: 'Radio',

        props: {
                group: {
                        type: String,
                        required: true,
                },
                label: {
                        type: String,
                        required: true,
                },
                id: {
                        type: String,
                        required: true,
                },
        },

        data() {
                return {
                        value: this.id,
                        isSelected: false,
                        isFocused: false,
                };
        },

        computed: {
                advancedOptions: () => store.getters.advancedOptions,
                radioGroup: () => store.getters.componentsRadio,
        },

        created() {
                // define current group
                let mygroup: { group: string; id: string[]; checked: boolean[] } = { group: this.group, id: [this.id], checked: [this.isSelected] };

                // register component with store
                store.commit('registerComponentRadioGroup', mygroup);
        },

        methods: {
                clicked: function() {
                        let mygroup: { group: string; id: string[]; checked: boolean[] } = { group: this.group, id: [this.id], checked: [true] };
                        store.commit('registerComponentRadioGroup', mygroup);
                }
        },

        watch: {
                radioGroup: function() {
                        // find corresponding group and id and set checked value
                        for(let i = 0; i < this.radioGroup.length; i++) {
                                if(this.radioGroup[i].group === this.group) {
                                        for(let j = 0; j < this.radioGroup[i].id.length; j++) {
                                                if(this.radioGroup[i].id[j] === this.id) this.isSelected = this.radioGroup[i].checked[j];
                                        }
                                }
                        }
                }
        }
});
</script>

<style scoped>
        .ba-radio {
                display: block;
                width: 1.75em;
                height: 1.75em;
                border-radius: 50%;
                border: 0.1em solid;
        }

        .ba-radio-checked {
                background: radial-gradient(rgba(26, 32, 44, 1) 50%,rgba(26, 32, 44, 0) 55%);
                background-position: 50%, 50%;
        }

        .ba-radio-focused {
                background: radial-gradient(rgba(26, 32, 44, .5) 50%,rgba(26, 32, 44, 0) 55%);
                background-position: 50%, 50%;
        }
</style>