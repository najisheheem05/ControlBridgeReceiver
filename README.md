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

### 3. Connect from ControlBridge (Android)

- Install [ControlBridge](https://github.com/najisheheem05/ControlBridge) on your phone
- Connect over Wi-Fi
- Start playing!

> **Version compatibility:** ControlBridgeReceiver and ControlBridge releases are paired.

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
- Firewall may need to allow the UDP port used for input streaming
- ViGEm is only required for Windows; Linux uses the in-kernel uinput API
- The Linux virtual pad reports itself as a wired Xbox 360 controller, so SDL, Steam and Wine/Proton use the standard Xbox mapping

---

## Credits

- **ViGEm** — Virtual Gamepad Emulation Framework for windows support
- Kotlin & Kotlin Multiplatform teams

---

## License

This project is licensed under the GNU General Public License v3.0 ([GPL-3.0-only](LICENSE)).
