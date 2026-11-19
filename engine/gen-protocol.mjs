// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Generates src/protocol.ts from protocol/schema.json (decision D8).

import { writeFile } from 'node:fs/promises';
import { URL } from 'node:url';

import { compileFromFile } from 'json-schema-to-typescript';

const banner = `// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Generated from protocol/schema.json by "npm run gen:protocol". Do not edit.
`;

const schemaUrl = new URL('../protocol/schema.json', import.meta.url);
const outputUrl = new URL('./src/protocol.ts', import.meta.url);

const source = await compileFromFile(schemaUrl.pathname, {
  bannerComment: '',
  additionalProperties: false,
});
await writeFile(outputUrl, `${banner}\n${source}`);
