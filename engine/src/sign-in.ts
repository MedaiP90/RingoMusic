// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Main-side sign-in: opens the site's own sign-in dialog and waits until the user signs in.
// `authorize()` is not used: the site resets its token from its own storage (see PROTOCOL.md).

import type { WindowOpenHandlerResponse } from 'electron';

import { ErrorCode } from './error-code.ts';
import { log } from './log.ts';
import type { AuthStatus, Notification } from './protocol.ts';
import { RpcError } from './rpc.ts';

const ALLOWED_POPUP_DOMAIN = 'apple.com';
// The site opens its sign-in dialog on this route.
const LOGIN_URL = 'https://music.apple.com/login';

export interface SignInWindow {
  loadURL(url: string): Promise<void>;
  show(): void;
  hide(): void;
}

export interface PageCaller {
  call(method: string, params: unknown): Promise<unknown>;
}

interface Waiter {
  resolve: () => void;
  reject: (error: unknown) => void;
}

// Only https pages of `apple.com` and its subdomains can open as login pop-ups.
export function isAllowedPopupUrl(url: string): boolean {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }
  const host = parsed.hostname;
  return (
    parsed.protocol === 'https:' &&
    (host === ALLOWED_POPUP_DOMAIN || host.endsWith(`.${ALLOWED_POPUP_DOMAIN}`))
  );
}

// Allowed pop-ups use the same partition as the main window. All other URLs open nothing.
export function decidePopup(url: string, partition: string): WindowOpenHandlerResponse {
  if (!isAllowedPopupUrl(url)) {
    return { action: 'deny' };
  }
  return {
    action: 'allow',
    overrideBrowserWindowOptions: {
      webPreferences: { partition, contextIsolation: true, sandbox: true, nodeIntegration: false },
    },
  };
}

export class SignIn {
  private active: Promise<null> | undefined;
  private waiter: Waiter | undefined;

  constructor(
    private readonly window: SignInWindow,
    private readonly page: PageCaller,
  ) {}

  // Returns when the user is authorized. Parallel calls share one login.
  run(): Promise<null> {
    this.active ??= this.execute().finally(() => {
      this.active = undefined;
      this.waiter = undefined;
    });
    return this.active;
  }

  // Ends the login with `cancelled`. Call it when the user closes the window.
  cancel(): void {
    this.waiter?.reject(new RpcError(ErrorCode.Cancelled, 'user closed the login window'));
  }

  // Ends the login with success when the page reports `authorized: true`.
  handleNotification(notification: Notification): void {
    if (notification.method === 'auth.changed' && notification.params.authorized) {
      this.waiter?.resolve();
    }
  }

  private async execute(): Promise<null> {
    const status = (await this.page.call('auth.status', undefined)) as AuthStatus;
    if (status.authorized) {
      return null;
    }
    await this.openLoginDialog();
    this.window.show();
    try {
      await new Promise<void>((resolve, reject) => {
        this.waiter = { resolve, reject };
      });
    } finally {
      this.window.hide();
    }
    return null;
  }

  // Loads the login route before the window shows, so the home page does not flash.
  // A failed load is not fatal: the user can still click the site's Sign In button.
  private async openLoginDialog(): Promise<void> {
    try {
      await this.window.loadURL(LOGIN_URL);
    } catch (error) {
      log.warn({ error: String(error) }, 'cannot load the login page');
    }
  }
}
