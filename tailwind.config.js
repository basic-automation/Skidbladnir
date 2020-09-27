/* eslint-disable global-require */
module.exports = {
        theme: {
                extend: {
                        transitionProperty: {
                                'height': 'height',
                                'width': 'width',
                                'size': 'width, height',
                                'spacing': 'margin, padding',
                        },
                },
        },
        variants: {},
        plugins: [],
        purge: [
                './src/**/*.html',
                './src/**/*.vue',
                './src/**/*.jsx',
        ],
};
