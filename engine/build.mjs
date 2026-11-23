// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import { build } from 'esbuild';

// Main and preload run in Node. Only electron stays external, so the install tree needs no node_modules.
const nodeTargets = {
  entryPoints: ['src/main.ts', 'src/preload.ts'],
  outdir: 'dist',
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node24',
  external: ['electron'],
};

// The hook runs in the page, so it must be one file without require or import.
const hookTarget = {
  entryPoints: ['src/hook.ts'],
  outdir: 'dist',
  bundle: true,
  platform: 'browser',
  format: 'iife',
  target: 'chrome140',
};

await Promise.all([build(nodeTargets), build(hookTarget)]);
