import { build } from 'esbuild';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
mkdirSync('dist', { recursive: true });
await build({entryPoints:['src/app.jsx'], bundle:true, outfile:'dist/app.js', minify:true, jsx:'automatic', jsxImportSource:'preact', target:'es2022'});
const css=readFileSync('dist/styles.css','utf8');
const js=readFileSync('dist/app.js','utf8').replaceAll('</script','<\\/script');
writeFileSync('dist/index.html',`<!doctype html><html lang="es"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; base-uri 'none'; form-action 'none'"><title>Vyntra Desktop</title><style>${css}</style></head><body><div id="app"></div><script>${js}</script></body></html>`);
