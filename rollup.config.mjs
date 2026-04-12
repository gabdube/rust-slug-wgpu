import typescript from '@rollup/plugin-typescript';

export default [
    {
        input: './rust-slug-demo/demo.ts',
        output: { file: './build/rust_slug_demo.js', format: 'es' },
        plugins: [typescript()]
    },
];
