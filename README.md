# OmaBeam

**No app. No account. No cloud. Scan and go.**

OmaBeam is a lightweight CLI for Omarchy that beams text, links, or one file
from your computer to a phone on the same local network using a QR code.

## Install

Until the AUR package is available, install the latest source revision with
Cargo:

```sh
cargo install --locked --git https://github.com/Choi-Jungwoo/omabeam
```

OmaBeam uses `wl-paste` from `wl-clipboard` for the Wayland clipboard and `ip`
from `iproute2` to find the local network address. Both are included with
Omarchy.

## Usage

Share the current clipboard:

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

Text and URLs are encoded directly in the terminal QR code. File sharing runs
in the foreground until you press <kbd>Ctrl</kbd>+<kbd>C</kbd>. Formats that the
phone browser supports are shown inline; other files are downloaded.

## Firewall

File sharing uses TCP port `61234`. If a firewall blocks the connection, allow
that port only from your trusted LAN. Replace these example network values:

```sh
interface=wlp0s20f3
subnet=192.168.1.0/24
sudo ufw allow in on "$interface" from "$subnet" to any port 61234 proto tcp
```

## Privacy

OmaBeam uploads nothing. The temporary server binds only to the current LAN
address, exposes only the selected regular file, and protects its URL with a
new random route token each time it runs.

## License

[MIT](LICENSE)
