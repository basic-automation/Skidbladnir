import { SelectPreset } from './SelectPreset';
import {SliderPreset } from './SliderPreset';

export interface Quality {
        [name: string]: SelectPreset[] | SliderPreset;
}