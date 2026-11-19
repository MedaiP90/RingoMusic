// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

// Electron main process: window, session, and socket client.

import path from 'node:path';

import { app, BrowserWindow, components } from 'electron';
import type { Session } from 'electron';

import pkg from '../package.json' with { type: 'json' };
import { forwardLogs, log } from './log.ts';
import { PageBridge } from './page-bridge.ts';
import { PAGE_METHODS } from './page-methods.ts';
import { RpcClient } from './rpc.ts';
import { decidePopup, SignIn } from './sign-in.ts';

const START_URL = 'https://music.apple.com';
const SESSION_PARTITION = 'persist:ringomusic';
const EXIT_CODE_BAD_CONFIGURATION = 2;

// Set when the app quits, so that the window can close.
let quitting = false;
app.on('before-quit', () => {
  quitting = true;
});

// The Chrome major version comes from the running Electron build.
function chromeMajorVersion(): string {
  return process.versions.chrome.split('.')[0] ?? '';
}

// Plain Chrome user agent: it has no "Electron" token.
function buildUserAgent(): string {
  return (
    'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) ' +
    `Chrome/${chromeMajorVersion()}.0.0.0 Safari/537.36`
  );
}

function buildClientHintBrands(): string {
  const major = chromeMajorVersion();
  return `"Chromium";v="${major}", "Google Chrome";v="${major}", "Not.A/Brand";v="99"`;
}

function readDataDirectory(): string {
  const dataDirectory = process.env['RINGO_ENGINE_DATA_DIR'];
  if (!dataDirectory) {
    process.stderr.write('RINGO_ENGINE_DATA_DIR is not set\n');
    process.exit(EXIT_CODE_BAD_CONFIGURATION);
  }
  return dataDirectory;
}

function readSocketPath(): string {
  const socketPath = process.env['RINGO_ENGINE_SOCKET'];
  if (!socketPath) {
    process.stderr.write('RINGO_ENGINE_SOCKET is not set\n');
    process.exit(EXIT_CODE_BAD_CONFIGURATION);
  }
  return socketPath;
}

// Connects to Rust, forwards logs, and quits the app when the socket closes.
async function connectToRust(): Promise<RpcClient> {
  const client = await RpcClient.connect(
    readSocketPath(),
    { engineVersion: pkg.version, electron: process.versions.electron },
    () => app.quit(),
  );
  forwardLogs((notification) => client.notify(notification));
  client.register('engine.shutdown', () => {
    // Reply first, then quit on the next tick.
    setImmediate(() => app.quit());
    return Promise.resolve(null);
  });
  return client;
}

// Sends page notifications to Rust and requests to the page. Sign-in runs in main because it drives the window.
function connectPageBridge(client: RpcClient, window: BrowserWindow): void {
  const bridge = new PageBridge(window.webContents, (notification) => {
    client.notify(notification);
    signIn.handleNotification(notification);
  });
  const signIn = new SignIn(window, bridge);
  for (const method of PAGE_METHODS) {
    client.register(method, (params) => bridge.call(method, params));
  }
  client.register('auth.signIn', () => signIn.run());
  // The login window hides, never closes. A close during sign-in cancels it.
  window.on('close', (event) => {
    if (!quitting) {
      event.preventDefault();
      signIn.cancel();
      window.hide();
    }
  });
  bridge.start();
}

// Replaces client hint brands that contain "Electron" on every request.
function hideElectronBrand(windowSession: Session): void {
  const brands = buildClientHintBrands();
  windowSession.webRequest.onBeforeSendHeaders((details, callback) => {
    const headers = details.requestHeaders;
    if (headers['Sec-CH-UA']?.includes('Electron')) {
      headers['Sec-CH-UA'] = brands;
    }
    callback({ requestHeaders: headers });
  });
}

async function waitForWidevine(): Promise<void> {
  await components.whenReady();
  log.info({ components: components.status() }, 'components ready');
}

function createWindow(userAgent: string): BrowserWindow {
  const window = new BrowserWindow({
    show: false,
    webPreferences: {
      backgroundThrottling: false,
      partition: SESSION_PARTITION,
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      preload: path.join(__dirname, 'preload.js'),
    },
  });
  window.webContents.setUserAgent(userAgent);
  window.webContents.setWindowOpenHandler(({ url }) => decidePopup(url, SESSION_PARTITION));
  window.webContents.on('did-finish-load', () => {
    log.info({ url: window.webContents.getURL() }, 'did-finish-load');
    void logPageBrands(window);
  });
  window.webContents.on('did-fail-load', (_event, code, description, url) => {
    log.error({ code, description, url }, 'did-fail-load');
  });
  return window;
}

// Reports the brands the page sees, to check that "Electron" is hidden.
async function logPageBrands(window: BrowserWindow): Promise<void> {
  const brands: unknown = await window.webContents.executeJavaScript(
    'JSON.stringify({ userAgent: navigator.userAgent, brands: navigator.userAgentData?.brands })',
  );
  log.info({ page: brands }, 'page identity');
}

async function start(): Promise<void> {
  const dataDirectory = readDataDirectory();
  const client = await connectToRust();
  app.setPath('userData', dataDirectory);
  app.commandLine.appendSwitch('autoplay-policy', 'no-user-gesture-required');
  const userAgent = buildUserAgent();
  app.userAgentFallback = userAgent;

  await app.whenReady();
  await waitForWidevine();

  const window = createWindow(userAgent);
  connectPageBridge(client, window);
  const windowSession = window.webContents.session;
  windowSession.setUserAgent(userAgent);
  hideElectronBrand(windowSession);
  if (process.env['RINGO_ENGINE_SHOW'] === '1') {
    window.show();
  }
  await window.loadURL(START_URL);
}

start().catch((error: unknown) => {
  log.fatal({ error }, 'engine failed to start');
  process.exit(1);
});

// Keep the process alive when no window is visible.
app.on('window-all-closed', () => {});
