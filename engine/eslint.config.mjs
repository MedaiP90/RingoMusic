// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist/', 'node_modules/', 'src/protocol.ts'] },
  js.configs.recommended,
  tseslint.configs.recommendedTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: { allowDefaultProject: ['eslint.config.mjs', 'build.mjs'] },
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  { files: ['**/*.mjs'], extends: [tseslint.configs.disableTypeChecked] },
);
