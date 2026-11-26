// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Methods that run in the page. Main handles `auth.signIn` and `engine.shutdown` alone. Main wraps `auth.signOut`: it also clears the cookies.

import type { Request } from './protocol.ts';

export const PAGE_METHODS = [
  'auth.status',
  'auth.signOut',
  'player.setQueue',
  'player.playNext',
  'player.playLater',
  'player.remove',
  'player.play',
  'player.pause',
  'player.stop',
  'player.next',
  'player.previous',
  'player.seek',
  'player.setVolume',
  'player.setRepeat',
  'api.get',
  'api.write',
] as const satisfies readonly Request['method'][];

export type PageMethod = (typeof PAGE_METHODS)[number];
