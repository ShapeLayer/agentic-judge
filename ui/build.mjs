import { build, context } from 'esbuild';
const options = { entryPoints: {app: 'app/app.ts', 'editor.worker': 'node_modules/monaco-editor/esm/vs/editor/editor.worker.js'}, bundle: true, format: 'esm', target: 'es2022', outdir: 'dist/app', minify: true, loader: {'.ttf': 'dataurl'} };
if (process.argv.includes('--watch')) { const ctx = await context(options); await ctx.watch(); } else { await build(options); }
