# Security Policy

## Supported Versions

ControlBridgeReceiver is under active development. Only the latest released version is supported with security fixes. Please make sure you're on the most recent release before reporting an issue.

| Version | Supported |
| ------- | --------- |
| Latest release | ✅ |
| Older releases | ❌ |

## Reporting a Vulnerability

If you discover a security vulnerability in ControlBridgeReceiver, please **do not open a public GitHub issue**. Instead:

1. Report it privately via GitHub's private vulnerability reporting feature on this repository (Security tab -> "Report a vulnerability").

Please include:
- A description of the vulnerability and its potential impact
- Steps to reproduce (proof-of-concept if possible)
- Affected version(s) of ControlBridgeReceiver and, if relevant, [ControlBridge](https://github.com/najisheheem05/ControlBridge)
- Your OS version, ViGEmBus version, and receiver build, if relevant to reproduction

You should receive an initial response within a few days. Reports are taken seriously.

## Scope and Known Risk Areas

ControlBridgeReceiver listens for **untrusted UDP input on the local network** and executes it to emulate a virtual controller. This makes certain areas particularly security sensitive:

- **UDP packet parsing** - malformed, malicious, or unexpected packets sent to the receiver's listening port should be handled safely (no crashes or memory corruption). This is the primary attack surface.
- **No built-in encryption or authentication on the input stream by default** so anyone on the same local network can potentially send UDP packets to the receiver's port, not just a legitimate [ControlBridge](https://github.com/najisheheem05/ControlBridge) client. Only run the receiver on networks you trust.
- **Virtual controller bindings** - bugs in the virtual controller emulation that mishandle attacker-influenced input (e.g. from a spoofed packet) are in scope.
- The receiver does not require internet access for core functionality; it only needs to be reachable on the local network from the ControlBridge Android client.

## Out of Scope

- Issues that require physical access to an already compromised machine
- Social engineering attacks
- Vulnerabilities in ViGEmBus itself, please report those to the [ViGEm project](https://github.com/nefarius/ViGEmBus) directly
- Vulnerabilities in third-party dependencies with no practical exploit path through ControlBridgeReceiver itself

## Disclosure

We ask that you give us a reasonable opportunity to investigate and fix a reported vulnerability before any public disclosure. Credit will be given in release notes for responsibly disclosed issues, unless you prefer to remain anonymous.
