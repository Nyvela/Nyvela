# X11 desktop

Lumen's graphical session boots the upstream X.Org server and Joe's Window
Manager (JWM). JWM provides window management, a panel, application menus
and workspaces. The session packages X.Org bitmap fonts and cursor assets,
an XKB keymap, and the upstream JWM example style.

## Build and launch

Prepare the sibling repositories and [native application toolchain](ports.md).
The `codex/native-desktop` branch requires the matching Lumsh branch.

On a build host with Python 3.11 or later, install the additional X11 build tools. On Ubuntu, use the packages below.

```sh
sudo apt install curl pkg-config meson ninja-build xfonts-utils x11-xkb-utils
```

From the Lumen checkout, run:

```sh
make run-desktop
```

The first build downloads and verifies the upstream releases listed in
`ports/x11/sources.lock` and cross-compiles them for Lumen. `make x11` builds
the initrd without starting QEMU. Set `X11_JOBS` to change build parallelism.
On WSL, setting `X11_WORK_DIR=$HOME/.cache/lumen-x11` keeps package work files
on the Linux filesystem.

1. Create an administrator account on the first boot. Use a lowercase
   username and a password of at least eight characters.
2. Sign in. Press **F1** or select **Open desktop** to enter the JWM session.
   **F2** adds a regular account from an administrator session. **F3** changes
   the active account's password.
3. Open the **JWM** menu at the bottom left to start Xedit or XClock. Drag
   a title bar to move a window. Use the panel to switch windows or workspaces.
4. Save files with Xedit in your home directory. Choose **JWM → Sign out** and
   confirm to save your home snapshot and return to the login screen.

The login manager stores accounts and home snapshots on the NVMe FAT32 image
at `build/fat32.img` by default. Home files are committed at sign-out. Keep
that disk across boots. `make clean` preserves the configured disk image.

## Display and applications

X.Org renders a 640×400 framebuffer and receives PS/2 events through Lumen's
native display calls. The X11 socket is local to the signed-in UID. The X11
build uses software rendering and a precompiled US keyboard layout.
XClock currently follows guest uptime.

JWM, Xedit and XClock are static Lumen ELF applications. The musl port provides
their POSIX interfaces over Lumen's native calls. JWM launches programs using
Lumen's `SPAWN` call. Menu commands use absolute `/bin/` paths with
space-separated arguments.

`make run-ports` starts the development shell for SQLite, Doom and other
command-line ports. `make run-lumdesk` opens the
[Lumdesk development session](lumdesk.md).
