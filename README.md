# ControlBridgeReceiver

[![Discord](https://img.shields.io/discord/1496412688685858846?label=&logo=discord&logoColor=ffffff&color=5865F2&labelColor=404EED)](https://discord.gg/BrMAZbEyXs)

> **Desktop receiver for ControlBridge** - turns UDP input from your phone into a real virtual gamepad

ControlBridgeReceiver is the Desktop side companion to **[ControlBridge](https://github.com/najisheheem05/ControlBridge)**. It listens for low latency controller input streamed from the ControlBridge Android app and exposes it to Desktop (and games) as a real virtual controller, using **ViGEm** on Windows.

This is one half of a two-part project:

- **[ControlBridge](https://github.com/najisheheem05/ControlBridge)** -> Android / client app (virtual controller UI)
- **ControlBridgeReceiver** -> Desktop / receiver app (creates the virtual controller) *(this repo)*

---

## How it works

```
[ Android Phone ] -- UDP --> [ ControlBridgeReceiver (Windows) ] --> [ ViGEm ] --> Game
```

1. **[ControlBridge](https://github.com/najisheheem05/ControlBridge) (Android)** renders a virtual controller, captures input, and streams it over UDP on the local WiFi network.
2. **ControlBridgeReceiver (Desktop)** listens for those UDP packets, executes the controller states, which exposes a virtual Xbox 360 controller to the OS. Supports up to 4 simultaneous players!

Games see it as a *real* controller.

---

## Features

- Low-latency **UDP** input receiving
- **Multi-device support**: Connect up to 4 devices simultaneously (Player 1 to 4)
- **Xbox 360** virtual controller support via ViGEm
- Built with **Rust** and **Slint UI**
- Works over local WiFi, no internet required
- Pairs with the [ControlBridge](https://github.com/najisheheem05/ControlBridge) Android app

---

## Requirements

- Windows (10 / 11)
- **ViGEmBus Driver** installed (required for virtual controller creation)
- The [ControlBridge](https://github.com/najisheheem05/ControlBridge) Android app running on the same local WiFi network

---

## Getting Started

### 1. Install ViGEmBus

Download and install **ViGEmBus** from the [official ViGEmBus GitHub releases](https://github.com/nefarius/ViGEmBus/releases).

### 2. Run ControlBridgeReceiver

```
ControlBridgeReceiver.exe
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

## Notes

- Works best on local WiFi
- Firewall may need to allow the UDP port used for input streaming
- ViGEm is only required for Windows

---

## Credits

- **ViGEm** — Virtual Gamepad Emulation Framework for windows support
- Kotlin & Kotlin Multiplatform teams

---

## License

This project is licensed under the GNU General Public License v3.0 ([GPL-3.0-only](LICENSE)).

---

> Built for low latency, simplicity, and fun.
