/* eslint-disable @typescript-eslint/no-var-requires */
/* eslint-disable global-require */
const colors = require('tailwindcss/colors')
module.exports = {
        theme: {
                extend: {
                        transitionProperty: {
                                'height': 'height',
                                'width': 'width',
                                'size': 'width, height',
                                'spacing': 'margin, padding',
                        },
                        minHeight: {
                                '0': '0',
                                '10': '2.5rem',
                                '20': '5rem',
                                'full': '100%',
                        },
                        minWidth: {
                                '20': '10rem',
                                '40': '20rem',
                        },
                },
                colors: {
                        gray: colors.blueGray,
                },
        },
        variants: {
                backgroundColor: ['responsive', 'hover', 'focus', 'disabled'],
                textColor: ['responsive', 'hover', 'focus', 'disabled'],
        },
        plugins: [],
        purge: [
                './src/**/*.html',
                './src/**/*.vue',
                './src/**/*.jsx',
        ],
};
