// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Splits a text stream into NDJSON lines. A partial line stays in the buffer.
export class LineFramer {
  private buffer = '';

  // Returns the complete, non-empty lines that the chunk finishes.
  push(chunk: string): string[] {
    const lines = (this.buffer + chunk).split('\n');
    this.buffer = lines.pop() ?? '';
    return lines.filter((line) => line.trim() !== '');
  }
}
