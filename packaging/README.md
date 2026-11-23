<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 RingoMusic contributors
-->

# Packaging notes

## Chromium sandbox

The engine is Electron, which uses the Chromium sandbox. The sandbox is
required on all native targets.

### Why `chrome-sandbox` is SUID root

`chrome-sandbox` is the helper that sets up the sandbox when the kernel does
not allow unprivileged user namespaces. It must have owner `root` and mode
`4755`. If the owner or the mode is wrong, Chromium stops with an error that
names `chrome-sandbox`.

### Why the AppArmor profile exists

Ubuntu 24.04 and later restrict unprivileged user namespaces with AppArmor.
Without a profile, the engine cannot create the namespace and the sandbox
fails. `packaging/apparmor/ringomusic` gives the engine binary the `userns`
permission and nothing else (`flags=(unconfined)`). The package scripts load
the profile. Check the syntax with:

```sh
apparmor_parser -Q -T packaging/apparmor/ringomusic
```

### `--no-sandbox` is forbidden

Never start the engine with `--no-sandbox`, also not in development (decision
D19). If the sandbox fails, fix the installation. When the engine exits
at start with a sandbox error, the supervisor reports
`ErrorCode::SandboxFailed`, does not restart the engine, and the app shows a
page with a link to the troubleshooting guide.
