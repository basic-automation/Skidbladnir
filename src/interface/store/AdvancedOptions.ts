import { Associations } from './Associations';
import { Mode } from './Mode';
import { Quality } from './Quality';
import { Compression } from './Compression';
import { Deblocking } from './Deblocking';
import { NoiseShaping } from './NoiseShaping';

export interface AdvancedOptions {
        isShown: boolean;
        associations: Associations;
        mode: Mode;
        quality: Quality;
        compression: Compression;
        deblocking: Deblocking;
        noiseshaping: NoiseShaping;
}