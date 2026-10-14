# Ultimate-NAG52 (Config app)

The configuration app for the Ultimate-NAG52 TCU

## Installation

### macOS

The macOS build supports Apple Silicon (M1 and newer) and requires macOS 11.0 or newer; Intel Macs are not supported.

#### ⚠️ IMPORTANT: App is not code-signed
This app is not signed with an Apple Developer Certificate. macOS will show a security warning when you first try to open it.

#### Installation Steps:
1. Download `config_app.dmg` from [Releases](https://github.com/rnd-ash/ultimate-nag52-config-app/releases)
2. Open the downloaded DMG file
3. Drag `Ultimate-NAG52-Config-App.app` to your **Applications** folder
4. Open the app once, then dismiss the security warning
5. Open **System Settings → Privacy & Security** (on macOS 11–12: **System Preferences → Security & Privacy → General**)
6. Click **Open Anyway** for this app, authenticate if prompted, and confirm **Open**

On macOS 15 and newer, Control-click → Open no longer bypasses Gatekeeper for unsigned or unnotarized apps. See [Apple's instructions](https://support.apple.com/en-us/102445).

**This is only needed the first time.** After that, you can launch the app normally.

<details>
<summary>Alternative: Using Terminal (for advanced users)</summary>

If you prefer using the terminal:
```bash
xattr -cr /Applications/Ultimate-NAG52-Config-App.app
open /Applications/Ultimate-NAG52-Config-App.app
```
</details>

### Windows
Run the `.exe` file directly.

### Linux
The published AppImage targets Linux x86_64. Download `config_app.AppImage` from [Releases](https://github.com/rnd-ash/ultimate-nag52-config-app/releases), then make it executable and run it:
```bash
chmod +x config_app.AppImage
./config_app.AppImage
```

#### File dialogs

Linux file dialogs use the XDG Desktop Portal backend by default. The desktop session needs `xdg-desktop-portal` with a compatible GTK, GNOME, or KDE backend, `libdbus`, and Zenity for fallback dialogs. These are usually installed with a full desktop environment; minimal sessions may need them installed separately. A wlroots-only portal backend does not provide the API this app uses. See the [`rfd` Linux backend requirements](https://docs.rs/rfd/0.17.2/rfd/#linux--bsd-backends).

#### USB permissions

The logged-in user needs read and write access to the TCU's serial device, usually `/dev/ttyUSB*` or `/dev/ttyACM*`. Check its group with:
```bash
ls -l /dev/ttyUSB* /dev/ttyACM*
```

Add your user to the group shown for the device (commonly `dialout` on Debian/Ubuntu or `uucp` on Arch Linux). For example:
```bash
sudo usermod -aG dialout "$USER"
```
Log out and back in for the new group membership to take effect. If the device uses a different group, substitute that group name.

## Starting version 1.0.8
to load the TCU Settings page, you will need `MODULE_SETTINGS.yml` file that is shipped with the TCUs firmware on Github!

## Repo structure

* Backend - Backend for ECU diagnostics
* config_app - Main configuration suite UI

## Branch names

The branches of this repository will follow the same branch names as the TCU firmware. Builds will only be avaliable for the main and dev branch. Other branches will need to be compiled manually
