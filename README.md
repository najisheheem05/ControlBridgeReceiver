<p align="center">
  <img src="icons/512x512.png" alt="ControlBridge Logo" width="100">
</p>

<h1 align="center"> ControlBridgeReceiver </h1>

> **Desktop receiver for ControlBridge** - turns UDP input from your phone into a real virtual gamepad

ControlBridgeReceiver is the Desktop side companion to **[ControlBridge](https://github.com/najisheheem05/ControlBridge)**. It listens for low latency controller input streamed from the ControlBridge Android app and exposes it to Desktop (and games) as a real virtual controller, using **ViGEm** on Windows and **uinput** on Linux.

This is one half of a two-part project:

- **[ControlBridge](https://github.com/najisheheem05/ControlBridge)** -> Android / client app (virtual controller UI)
- **ControlBridgeReceiver** -> Desktop / receiver app (creates the virtual controller) _(this repo)_

---

## How it works

```
[ Android Phone ] -- UDP --> [ ControlBridgeReceiver ] --> [ ViGEm (Windows) / uinput (Linux) ] --> Game
```

1. **[ControlBridge](https://github.com/najisheheem05/ControlBridge) (Android)** renders a virtual controller, captures input, and streams it over UDP on the local WiFi network.
2. **ControlBridgeReceiver (Desktop)** listens for those UDP packets, executes the controller states, which exposes a virtual Xbox 360 controller to the OS. Supports up to 4 simultaneous players!

Games see it as a _real_ controller.

---

## Features

- Low-latency **UDP** input receiving
- **Multi-device support**: Connect up to 4 devices simultaneously (Player 1 to 4)
- **Xbox 360** virtual controller support via ViGEm (Windows) or uinput (Linux)
- **Rumble / force feedback** forwarded from games back to the phone
- Built with **Rust** and **Slint UI**
- Works over local WiFi, no internet required
- Pairs with the [ControlBridge](https://github.com/najisheheem05/ControlBridge) Android app

---

## Requirements

- **Windows 10 / 11** with the **ViGEmBus Driver** installed, or **Linux** with the kernel `uinput` module
- The [ControlBridge](https://github.com/najisheheem05/ControlBridge) Android app running on the same local WiFi network

---

## Getting Started

### Windows: install ViGEmBus

Download and install **ViGEmBus** from the [official ViGEmBus GitHub releases](https://github.com/nefarius/ViGEmBus/releases).

### Linux: enable uinput

Load the kernel module (most desktop distributions ship it and load it automatically):

```sh
sudo modprobe uinput
```

The receiver needs read/write access to `/dev/uinput`. On a normal desktop session systemd grants this through uaccess. If access is denied, add a udev rule:

```udev
KERNEL=="uinput", SUBSYSTEM=="misc", OPTIONS+="static_node=uinput", TAG+="uaccess"
```

Then reload the rules and re-connect the module:

```sh
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo modprobe -r uinput && sudo modprobe uinput
```

### Run ControlBridgeReceiver

```
ControlBridgeReceiver        # Linux
ControlBridgeReceiver.exe    # Windows
```

This starts listening for UDP input and creates virtual controllers dynamically as devices connect.

### Connect from ControlBridge (Android)

- Install [ControlBridge](https://github.com/najisheheem05/ControlBridge) on your phone
- Connect over Wi-Fi
- Start playing!

> If the receiver is not showing up on the phone, see the [firewall section](#firewall).

> **Version compatibility:** ControlBridgeReceiver and ControlBridge releases are paired.

---

## Firewall

If the phone shows no receiver, the desktop firewall is most likely blocking
the two UDP ports the app uses: `8083` for discovery and `8082` for input.

### Check if the firewall is active

- **firewalld:** `systemctl is-active firewalld`
- **ufw:** `sudo ufw status`
- **Windows:** Windows Security -> Firewall & network protection

### Deactivate it for testing

- **firewalld:** `sudo systemctl stop firewalld`
- **ufw:** `sudo ufw disable`
- **Windows:** turn off the firewall for the **Private** network

### Re-enable it after testing

- **firewalld:** `sudo systemctl start firewalld`
- **ufw:** `sudo ufw enable`
- **Windows:** turn the firewall back on

### Why this happens

The phone sends a UDP broadcast to `8083` to find the receiver, then streams
controller input to `8082`. If either port is blocked, the phone cannot detect
the receiver or the controller does not move. The protocol is unauthenticated:
any device on the same network that can reach these ports can send controller
input. Do not port-forward them or disable the firewall on public Wi-Fi. On your
own network, prefer allowing only the phone's IP:

```sh
# firewalld
sudo firewall-cmd --permanent --zone=public --add-rich-rule='rule family="ipv4" source address="PHONE_IP/32" port port="8083" protocol="udp" accept'
sudo firewall-cmd --permanent --zone=public --add-rich-rule='rule family="ipv4" source address="PHONE_IP/32" port port="8082" protocol="udp" accept'
sudo firewall-cmd --reload

# ufw
sudo ufw allow from PHONE_IP to any port 8083 proto udp
sudo ufw allow from PHONE_IP to any port 8082 proto udp
```

These rules are low risk: only these two UDP ports are opened, and only to the
phone's IP. The main risk is the IP changing: if DHCP reassigns `PHONE_IP` to
another device (or the phone NATs other devices behind that address), that
device could send controller input. Reserve a fixed lease for the phone, keep
the source as a single IP rather than a whole subnet, and remove the rules when
you stop using the receiver.

If it still fails, make sure the phone and desktop are on the same network and
that the router/hotspot does not have **client isolation** enabled.

---

## Supported Inputs

- Buttons: A / B / X / Y
- Shoulder buttons
- Triggers
- Analog Sticks
- DPAD buttons
- Rumble / force feedback

## Notes

- Works best on local WiFi
- The desktop firewall must allow inbound UDP `8082` and `8083` from the phone; see the [firewall section](#firewall) if the receiver is not detected
- ViGEm is only required for Windows; Linux uses the in-kernel uinput API
- The Linux virtual pad reports itself as a wired Xbox 360 controller, so SDL, Steam and Wine/Proton use the standard Xbox mapping
- The protocol is unauthenticated by design for low latency; keep the firewall rules limited to your phone and never expose the ports to the internet

---

## Credits

- **ViGEm** — Virtual Gamepad Emulation Framework for windows support
- Kotlin & Kotlin Multiplatform teams

---

## License

This project is licensed under the GNU General Public License v3.0 ([GPL-3.0-only](LICENSE)).
