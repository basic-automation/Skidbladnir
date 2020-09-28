<template>
        <button ref="button" class="flex bg-gray-500 flex-col w-full md:w-auto min-w-20 items-center cursor-pointer flex-shrink-0 focus:outline-none transition-all duration-300 ease-in-out" @click="clicked(); isFocused = false" @focus="isFocused = true" @blur="isFocused = false" @mouseover="isHover = true" @mouseout="isHover = false" :class="{ 'rounded-top-20px': isHover, 'rounded-20px': !isHover }">
                <div class="flex h-10 flex-row items-center justify-center w-full cursor-pointer flex-shrink-0">
                        <label class="flex ml-4 justify-start items-center h-full text-xs cursor-pointer antialiased flex-shrink-0">{{ label }}</label>
                        <div class="flex-1"></div>
                        <div class="ba-radio ml-4 mr-2 border-gray-900 cursor-pointer antialiased flex-shrink-0" :class="{ 'ba-radio-checked': isSelected, 'ba-radio-focused': isFocused }"></div>
                </div>
                <div :class="{ 'hidden': !isHover, 'w-full': isHover }"><hr class="flex border-gray-900 mx-4 mt-4 h-1" /></div>
                <div ref="info" class="transition-size duration-500" :class="[ {'h-0px': !isHover, 'w-full py-4': isHover }, isHover ? infoHeightClass : '' ]"><p class="text-xs text-justify" :class="[ { 'mx-2': isHover } ]">{{ infoLabel }}</p></div>
        </button>
        <input class="hidden" type="radio" :id="id" :name="group" :checked="isSelected" :value="value">
</template>

<script lang="ts">
/* eslint-disable no-unused-vars */
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
                        isHover: true,
                        buttonWidth: 'auto',
                        infoLabel: '',
                        info: null as HTMLDivElement | null,
                        infoHeightClass: this.id + '-height',
                        infoHeightVarName: '--' + this.id + '-height',
                        infoCSSID: 'ba-' + this.id
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

                // set infor label from store
                const mode = this.advancedOptions.mode;
                for(const [key, value] of Object.entries(mode)) {
                        if(typeof value === 'object') {
                                let newVal = value as { checked: boolean; label: string };
                                if(key === this.id) {
                                        this.infoLabel = newVal.label;
                                }
                        }
                }

                // set custom css height
                const heightVar = ':root { --' + this.id + '-height: auto; }';
                this.addCss(this.infoCSSID,heightVar);
                const setClass = '.' + this.infoHeightClass + ' { height: var(' + this.infoHeightVarName + '); }';
                this.addCss(this.infoCSSID, setClass);

        },

        mounted() {
                this.info = this.$refs.info as HTMLDivElement;
                this.logInfoHeight();
        },

        methods: {
                clicked: function() {
                        let mygroup: { group: string; id: string[]; checked: boolean[] } = { group: this.group, id: [this.id], checked: [true] };
                        store.commit('registerComponentRadioGroup', mygroup);
                        store.dispatch('syncModeWithRadioGroup', { group: this.group, id: this.id });
                },

                logInfoHeight: function() {

                        // eslint-disable-next-line @typescript-eslint/no-unused-vars
                        let inter = setInterval(() => {
                                if(this.info !== null) {
                                        console.log(this.info.clientHeight);
                                        if(this.info.clientHeight > 50){
                                                const root = document.documentElement;
                                                root.style.setProperty(this.infoHeightVarName, this.info.clientHeight + "px");
                                                this.isHover = false;
                                                clearInterval(inter);
                                        }
                                }
                        }, 1000);
                },

                addCss: function(nodeID: string, cssCode: string) {
                        if(!document.getElementById(nodeID)){
                                const styleElement = document.createElement('style') as HTMLStyleElement;
                                styleElement.type = 'text/css';
                                styleElement.id = nodeID;
                                styleElement.appendChild(document.createTextNode(cssCode));
                                document.getElementsByTagName('head')[0].appendChild(styleElement);
                        } else {
                                const styleElement = document.getElementById(nodeID) as HTMLStyleElement;
                                styleElement.type = 'text/css';
                                styleElement.id = nodeID;
                                let children = styleElement.childNodes;
                                for(let i = 0; i < children.length; i++) {
                                        console.log('Node: ' + children[i].nodeValue + '  value: ' + children[i].nodeValue)
                                        children[i].remove
                                }
                                styleElement.appendChild(document.createTextNode(cssCode));
                                document.getElementsByTagName('head')[0].appendChild(styleElement);
                        }
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
                },
                advancedOptions: function() {
                        console.log('advancedOptions running.');
                        const mode = this.advancedOptions.mode;
                        for(const [key, value] of Object.entries(mode)) {
                                if(typeof value === 'object') {
                                        let newVal = value as { checked: boolean; label: string }
                                        if(key === this.id) {
                                                this.infoLabel = newVal.label
                                        }
                                }
                        }
                },

                ['info.clientHeight']: function(newVal, oldVal) {
                        console.log(this.id + 'oldVal: ', oldVal, ' newVal: ', newVal);
                        if(newVal > 50) {
                                const heightVar = ':root { --' + this.id + '-height: ' + newVal + 'px; }';
                                this.addCss(this.infoCSSID,heightVar);
                                const setClass = '.' + this.infoHeightClass + ' { height: var(' + this.infoHeightVarName + '); }';
                                this.addCss(this.infoCSSID, setClass);
                        }
                }
        }
});
</script>

<style scoped>

        :root {
                --info-height: auto;
        }

        .info-height {
                height: var(--info-height);
        }

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

        .rounded-20px {
                border-radius: 20px;
        }

        .rounded-top-20px {
                border-top-right-radius: 20px;
                border-top-left-radius: 20px;
        }

        .h-0px {
                height: 0px;
        }

        .w-0px {
                width: 0px;
        }

        .w-330px {
                width: calc(330px - 4rem);
        }

        .scrollbar-hidden::-webkit-scrollbar {
                display: none;
        }
        
        .max-w-330px {
                max-width: calc(330px - 4rem);
        }
</style>