// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Generated from protocol/schema.json by "npm run gen:protocol". Do not edit.

/**
 * Notification envelope.
 */
export type Notification = {
  /**
   * Always `"2.0"`.
   */
  jsonrpc: "2.0";
} & (
  | {
      method: "engine.hello";
      params: {
        /**
         * Version of Electron.
         */
        electron: string;
        /**
         * Version of the engine package.
         */
        engine_version: string;
        /**
         * Protocol version of the engine.
         */
        proto: number;
      };
    }
  | {
      method: "engine.ready";
    }
  | {
      method: "engine.incompatible";
      params: {
        /**
         * Name of the failed check.
         */
        check: string;
        /**
         * Details of the failure.
         */
        detail: string;
      };
    }
  | {
      method: "engine.log";
      params: {
        /**
         * Structured fields of the record.
         */
        fields: {
          [k: string]: unknown;
        };
        /**
         * Severity of the record.
         */
        level: "trace" | "debug" | "info" | "warn" | "error";
        /**
         * Log message.
         */
        message: string;
      };
    }
  | {
      method: "auth.changed";
      params: {
        /**
         * The user authorized the app.
         */
        authorized: boolean;
      };
    }
  | {
      method: "player.state";
      params: {
        /**
         * New playback state.
         */
        state: "playing" | "paused" | "stopped" | "loading" | "ended";
      };
    }
  | {
      method: "player.nowPlaying";
      params: {
        /**
         * Album name.
         */
        album: string;
        /**
         * Catalog or library ID of the album, if MusicKit knows it.
         */
        album_id?: string | null;
        /**
         * Artist name.
         */
        artist: string;
        /**
         * Artwork URL, if the item has artwork.
         */
        artwork_url?: string | null;
        /**
         * Duration in milliseconds.
         */
        duration_ms: number;
        /**
         * Item ID.
         */
        id: string;
        /**
         * Index of the item in the queue that Rust sent.
         */
        queue_index: number;
        /**
         * Item title.
         */
        title: string;
      };
    }
  | {
      method: "player.position";
      params: {
        /**
         * Position in milliseconds.
         */
        position_ms: number;
      };
    }
  | {
      method: "error";
      params: {
        /**
         * Error code.
         */
        code: number;
        /**
         * If `true`, the engine quits after this notification.
         */
        fatal: boolean;
        /**
         * Error message.
         */
        message: string;
      };
    }
);
/**
 * Request envelope.
 */
export type Request = {
  /**
   * Request ID, unique in the connection.
   */
  id: number;
  /**
   * Always `"2.0"`.
   */
  jsonrpc: "2.0";
} & (
  | {
      method: "auth.status";
    }
  | {
      method: "auth.signIn";
    }
  | {
      method: "auth.signOut";
    }
  | {
      method: "player.setQueue";
      params: {
        /**
         * Catalog or library IDs, in play order.
         */
        items: string[];
        /**
         * Kind of the IDs in `items`.
         */
        kind: "songs";
        /**
         * Index in `items` of the first item to play.
         */
        start_index: number;
      };
    }
  | {
      method: "player.playNext";
      params: {
        /**
         * Catalog or library IDs, in play order.
         */
        items: string[];
        /**
         * Kind of the IDs in `items`.
         */
        kind: "songs";
      };
    }
  | {
      method: "player.playLater";
      params: {
        /**
         * Catalog or library IDs, in play order.
         */
        items: string[];
        /**
         * Kind of the IDs in `items`.
         */
        kind: "songs";
      };
    }
  | {
      method: "player.remove";
      params: {
        /**
         * Index in the queue that the engine plays.
         */
        index: number;
      };
    }
  | {
      method: "player.play";
    }
  | {
      method: "player.pause";
    }
  | {
      method: "player.stop";
    }
  | {
      method: "player.next";
    }
  | {
      method: "player.previous";
    }
  | {
      method: "player.seek";
      params: {
        /**
         * New position in milliseconds.
         */
        position_ms: number;
      };
    }
  | {
      method: "player.setVolume";
      params: {
        /**
         * Volume from 0.0 to 1.0.
         */
        volume: number;
      };
    }
  | {
      method: "player.setRepeat";
      params: {
        /**
         * New repeat mode.
         */
        mode: "none" | "one" | "all";
      };
    }
  | {
      method: "api.get";
      params: {
        /**
         * API path, for example `/v1/catalog/us/search`.
         */
        path: string;
        /**
         * Query parameters.
         */
        query: {
          [k: string]: string;
        };
      };
    }
  | {
      method: "engine.shutdown";
    }
);

/**
 * Root type that only exists to put all protocol definitions in one schema.
 * It is never sent on the wire.
 */
export interface ProtocolSchema {
  auth_status: AuthStatus;
  error_response: ErrorResponse;
  /**
   * All notifications from the engine to Rust.
   */
  from_engine:
    | {
        method: "engine.hello";
        params: {
          /**
           * Version of Electron.
           */
          electron: string;
          /**
           * Version of the engine package.
           */
          engine_version: string;
          /**
           * Protocol version of the engine.
           */
          proto: number;
        };
      }
    | {
        method: "engine.ready";
      }
    | {
        method: "engine.incompatible";
        params: {
          /**
           * Name of the failed check.
           */
          check: string;
          /**
           * Details of the failure.
           */
          detail: string;
        };
      }
    | {
        method: "engine.log";
        params: {
          /**
           * Structured fields of the record.
           */
          fields: {
            [k: string]: unknown;
          };
          /**
           * Severity of the record.
           */
          level: "trace" | "debug" | "info" | "warn" | "error";
          /**
           * Log message.
           */
          message: string;
        };
      }
    | {
        method: "auth.changed";
        params: {
          /**
           * The user authorized the app.
           */
          authorized: boolean;
        };
      }
    | {
        method: "player.state";
        params: {
          /**
           * New playback state.
           */
          state: "playing" | "paused" | "stopped" | "loading" | "ended";
        };
      }
    | {
        method: "player.nowPlaying";
        params: {
          /**
           * Album name.
           */
          album: string;
          /**
           * Catalog or library ID of the album, if MusicKit knows it.
           */
          album_id?: string | null;
          /**
           * Artist name.
           */
          artist: string;
          /**
           * Artwork URL, if the item has artwork.
           */
          artwork_url?: string | null;
          /**
           * Duration in milliseconds.
           */
          duration_ms: number;
          /**
           * Item ID.
           */
          id: string;
          /**
           * Index of the item in the queue that Rust sent.
           */
          queue_index: number;
          /**
           * Item title.
           */
          title: string;
        };
      }
    | {
        method: "player.position";
        params: {
          /**
           * Position in milliseconds.
           */
          position_ms: number;
        };
      }
    | {
        method: "error";
        params: {
          /**
           * Error code.
           */
          code: number;
          /**
           * If `true`, the engine quits after this notification.
           */
          fatal: boolean;
          /**
           * Error message.
           */
          message: string;
        };
      };
  notification: Notification;
  request: Request;
  success_response: SuccessResponse;
  /**
   * All methods from Rust to the engine.
   */
  to_engine:
    | {
        method: "auth.status";
      }
    | {
        method: "auth.signIn";
      }
    | {
        method: "auth.signOut";
      }
    | {
        method: "player.setQueue";
        params: {
          /**
           * Catalog or library IDs, in play order.
           */
          items: string[];
          /**
           * Kind of the IDs in `items`.
           */
          kind: "songs";
          /**
           * Index in `items` of the first item to play.
           */
          start_index: number;
        };
      }
    | {
        method: "player.playNext";
        params: {
          /**
           * Catalog or library IDs, in play order.
           */
          items: string[];
          /**
           * Kind of the IDs in `items`.
           */
          kind: "songs";
        };
      }
    | {
        method: "player.playLater";
        params: {
          /**
           * Catalog or library IDs, in play order.
           */
          items: string[];
          /**
           * Kind of the IDs in `items`.
           */
          kind: "songs";
        };
      }
    | {
        method: "player.remove";
        params: {
          /**
           * Index in the queue that the engine plays.
           */
          index: number;
        };
      }
    | {
        method: "player.play";
      }
    | {
        method: "player.pause";
      }
    | {
        method: "player.stop";
      }
    | {
        method: "player.next";
      }
    | {
        method: "player.previous";
      }
    | {
        method: "player.seek";
        params: {
          /**
           * New position in milliseconds.
           */
          position_ms: number;
        };
      }
    | {
        method: "player.setVolume";
        params: {
          /**
           * Volume from 0.0 to 1.0.
           */
          volume: number;
        };
      }
    | {
        method: "player.setRepeat";
        params: {
          /**
           * New repeat mode.
           */
          mode: "none" | "one" | "all";
        };
      }
    | {
        method: "api.get";
        params: {
          /**
           * API path, for example `/v1/catalog/us/search`.
           */
          path: string;
          /**
           * Query parameters.
           */
          query: {
            [k: string]: string;
          };
        };
      }
    | {
        method: "engine.shutdown";
      };
}
/**
 * Result of `auth.status`.
 */
export interface AuthStatus {
  /**
   * The user authorized the app.
   */
  authorized: boolean;
  /**
   * Storefront ID, for example `us`.
   */
  storefront: string;
}
/**
 * Error answer envelope.
 */
export interface ErrorResponse {
  error: ErrorObject;
  /**
   * ID of the request, or `null` if the engine could not read the request.
   */
  id?: number | null;
  /**
   * Always `"2.0"`.
   */
  jsonrpc: "2.0";
}
/**
 * Error details.
 */
export interface ErrorObject {
  /**
   * Error code.
   */
  code: number;
  /**
   * Optional extra data.
   */
  data?: unknown;
  /**
   * Error message.
   */
  message: string;
}
/**
 * Success answer envelope.
 */
export interface SuccessResponse {
  /**
   * ID of the request.
   */
  id: number;
  /**
   * Always `"2.0"`.
   */
  jsonrpc: "2.0";
  /**
   * Result of the method. Its shape depends on the method.
   */
  result: unknown;
}
