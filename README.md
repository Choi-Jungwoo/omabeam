# OmaBeam

**No app. No account. No cloud. Scan and go.**

OmaBeam is a lightweight CLI for Omarchy that beams clipboard text, images, or
copied files from your computer to a phone on the same local network using a QR
code.

## Install

Install OmaBeam through `mise`, included with Omarchy:

```sh
mise use --global cargo:omabeam
omabeam setup
```

OmaBeam uses `wl-paste` from `wl-clipboard` for the Wayland clipboard and `ip`
from `iproute2` to find the local network address. Both are included with
Omarchy.

`omabeam setup` adds OmaBeam to the Omarchy Share menu, binds `Super+B`, and
allows TCP port `61234` through UFW only for the active LAN. Run it again after
updating OmaBeam to refresh the managed menu and shortcut entries.

## Usage

Share the current clipboard text, image, or copied file:

```sh
omabeam
```

Share piped text or a URL:

```sh
echo "hello" | omabeam
```

Share one file:

```sh
omabeam photo.jpg
```

Text and URLs that fit are encoded directly in the terminal QR code. Longer
clipboard text, clipboard images, copied files, and explicit file arguments use
a temporary local server that runs until you press any key in an interactive
terminal. Formats that the phone browser supports are shown inline; other files
are downloaded.

## Privacy

OmaBeam uploads nothing. The temporary server binds only to the current LAN
address, exposes only the selected regular file or clipboard image, and
protects its URL with a new random route token each time it runs.

## License

[MIT](LICENSE)
