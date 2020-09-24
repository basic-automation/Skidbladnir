module.exports = {
        runtimeCompiler: process.env.NODE_ENV === 'development',
        pluginOptions: {
                electronBuilder: {
                        nodeIntegration: true,
                }
        }
}