import { RadioPreset } from './RadioPreset';
import { SliderPreset } from './SliderPreset';

export interface Compression {
        [name: string]: RadioPreset | SliderPreset;
}