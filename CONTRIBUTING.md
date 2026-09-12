# Contributing to ControlBridgeReceiver

Thanks for your interest in contributing to ControlBridgeReceiver, the desktop receiver that turns UDP input from the [ControlBridge](https://github.com/najisheheem05/ControlBridge) Android app into a virtual gamepad.

## Project Overview

ControlBridgeReceiver is the **desktop** half of the ControlBridge project. It listens for UDP packets from the ControlBridge Android app, deserializes controller states, and feeds them into ViGEm on Windows to expose a virtual Xbox 360 controller to the OS (supporting up to 4 simultaneous players).

```
[ Android Phone ] -- UDP --> [ ControlBridgeReceiver ] -- ViGEm --> Game
```

If your change involves the Android client (touch UI, input capture, layout editor), please open your PR against [ControlBridge](https://github.com/najisheheem05/ControlBridge) instead.

## Tech Stack

- **Rust** (2024 edition)
- **Slint** (Declarative GUI)
- **ViGEmClient** (Virtual Gamepad Emulation Framework for Windows)
- **UDP networking** (Low-latency input receiving & discovery)

## Getting Started

### Prerequisites

- Rust toolchain (`rustup`, stable)
- **Windows 10/11** with [ViGEmBus driver](https://github.com/nefarius/ViGEmBus/releases) installed, if testing controller emulation
- A device or emulator running [ControlBridge](https://github.com/najisheheem05/ControlBridge) to test against

### Setup

1. Fork the repository and clone your fork:
   ```bash
   git clone https://github.com/username/ControlBridgeReceiver.git
   cd ControlBridgeReceiver
   ```
2. Build and run the desktop app:
   ```bash
   cargo run
   ```

### Testing end to end

To verify input is actually reaching a virtual controller, you will need:
1. ViGEmBus installed (required for Windows).
2. The [ControlBridge](https://github.com/najisheheem05/ControlBridge) Android app running on the same local network.
3. A way to inspect gamepad input (e.g. Windows' "Set up USB game controllers" panel (`joy.cpl`), or a game).

## How to Contribute

### Reporting Bugs

Open an issue and include:
- OS and version (e.g. Windows 10/11)
- ControlBridgeReceiver version
- ControlBridge (Android) version
- Whether ViGEmBus is installed
- Steps to reproduce, and logs/console output if available

### Submitting Code Changes

1. Create a branch off `master`:
   ```bash
   git checkout -b feature/short-description
   ```
2. Make your changes and format your code:
   ```bash
   cargo fmt
   cargo check
   ```
3. Test on the target platform (Windows for ViGEm verification).
4. Commit with a clear, descriptive message.
5. Push to your fork and open a pull request against `master`.

## License

By contributing, you agree that your contributions will be licensed under the project's [GPL-3.0-only license](LICENSE).
