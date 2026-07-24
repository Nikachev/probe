# Documentation Index — rusty-probe-nicenano

Welcome to the technical documentation directory for the **rusty-probe-nicenano** firmware project.

---

## 📚 Guides & Specifications

| Document | Description |
|---|---|
| 📐 **[Architecture Overview](ARCHITECTURE.md)** | Technical specification of the firmware, RTIC 2 tasks, memory map (`0x26000`), SWD driver (`swd.rs`), PAC register I/O, and USB stack design. |
| 🧪 **[HIL Testing Guide](HIL_TESTING.md)** | Comprehensive guide for automated 43-test hardware test suite using two nice!nano v2 boards. |
| 🛠️ **[Diagnostic Utilities](DIAGNOSTICS.md)** | Guide to standalone diagnostic binaries ([`diag`](../src/bin/diag.rs) and [`diag_usb`](../src/bin/diag_usb.rs)) for LED blink codes and USB troubleshooting. |

---

## 🚀 Quick Links
- **[Main Project README](../README.md)**
- **[Source Code Directory](../src)**
- **[HIL Test Scripts](../tools)**
