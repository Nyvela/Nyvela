# Lumdesk

Lumdesk is a userspace desktop and login manager. Its compositor draws to
the QEMU Standard VGA framebuffer and handles PS/2 mouse and keyboard events.
A separate process supplies the Clock window through Lumen IPC.

## Start a desktop

From the Lumen checkout, build the [native application toolchain and ports](ports.md)
and run:

```sh
make run-desktop
```

When using the `codex/native-desktop` development branch, use the matching
branch in the sibling Lumsh checkout for desktop startup.

Use the QEMU window for input. On the first boot, create an administrator
account using a lowercase username and a password of at least eight
characters. On subsequent boots, sign in with that account or a user added
by the administrator. Press **F1** or choose **Open desktop** after signing
in. The session menu also offers **F3** to change your password.
Administrators can add regular accounts with **F2**.

The system login manager saves password verifiers and home snapshots on the
NVMe FAT32 image, `build/fat32.img` by default. Passwords use salted
PBKDF2-HMAC-SHA256. Home snapshots are written on successful sign-out.
Keep the same disk image across boots to retain your accounts and files.
`make clean` preserves the configured FAT32 and NVMe disk images.

## Use the desktop

- Click the taskbar to open **Files**, **Notes**, **Terminal**, **Clock** or
  **Doom**. **F1** through **F4** open the first four, and **F9** runs Doom.
- Drag a title bar to move a window. Drag its lower-right corner to resize
  it. Use the title bar buttons to maximize or close it. **Tab** cycles
  focus, **F11** maximizes or restores, and **Ctrl+W** closes the active window.
- **Files** browses your home and previews text. Select an entry with the
  mouse or arrow keys, press **Enter** to open it, and press **Backspace**
  to go up. **F5** refreshes the folder.
- **Notes** edits `notes.txt` in your home. Press **Ctrl+S** to save into
  RAMFS. Sign out to commit it to the NVMe snapshot.
- **Terminal** provides `ls`, `cat`, `mkdir`, `whoami`, and `run` for
  batch programs. It captures child output. For example, enter
  `run hello-std "from Lumdesk"` or `run sqlite3 demo.db "select 42;"`.
- Press **F10** or choose **Sign out** to save home files and return to login.

Doom takes the display for the duration of the game. Press **F10** followed
by **Y** inside Doom to return to the desktop.

## Session boundaries

Regular ELF applications inherit their parent’s user ID. Relative paths
resolve within that user’s home. The kernel limits regular accounts to their
own home, shared read-only programs in `/bin`, and the bundled game data.
The compositor owns the display seat and can delegate it to a child such as
Doom. A child of the graphical terminal receives EOF on stdin.

Lumdesk supports up to 16 accounts and home snapshots up to 4 MiB per user.
It currently backs up home directories on sign-out. The FAT32 account and
home files rely on the host and virtual disk for physical access control.
`make run` and `make run-ports` start the trusted development console.
Use `make run-desktop` for a login-gated local session.
