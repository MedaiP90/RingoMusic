// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Types for the parts of MusicKit JS and of the preload bridge that the hook uses.

interface MusicKitArtwork {
  // URL template with `{w}` and `{h}` placeholders.
  url?: string;
}

interface MusicKitMediaItem {
  id: string;
  title?: string;
  artistName?: string;
  albumName?: string;
  // Duration in milliseconds.
  playbackDuration?: number;
  artwork?: MusicKitArtwork;
  attributes?: {
    name?: string;
    artistName?: string;
    albumName?: string;
    durationInMillis?: number;
    artwork?: MusicKitArtwork;
  };
}

interface MusicKitEventMap {
  playbackStateDidChange: { state: number };
  nowPlayingItemDidChange: { item?: MusicKitMediaItem | null };
  // Both times are in seconds.
  playbackTimeDidChange: { currentPlaybackTime: number };
  authorizationStatusDidChange: { authorizationStatus: number };
  mediaPlaybackError: { message?: string };
}

interface MusicKitInstance {
  isAuthorized: boolean;
  nowPlayingItem?: MusicKitMediaItem | null;
  nowPlayingItemIndex: number;
  api: { music: (path: string, query?: object) => Promise<unknown> };
  addEventListener<K extends keyof MusicKitEventMap>(
    name: K,
    listener: (event: MusicKitEventMap[K]) => void,
  ): void;
}

interface RingoBridge {
  send(message: unknown): void;
  onRequest(callback: (request: unknown) => void): void;
}

interface Window {
  MusicKit?: { getInstance(): MusicKitInstance | undefined };
  ringoBridge?: RingoBridge;
  // Set after the hook runs once in this page.
  __ringoHookInjected?: boolean;
}
