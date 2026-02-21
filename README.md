# reedserial

A [Reedline](https://github.com/nushell/reedline)-based interactive shell for serial port devices, designed for use with [lisp-pid](https://github.com/jig/lisp-pid) running on RP2040/RP2350 microcontrollers.

## Features

- **Rich line editing** — Emacs keybindings, persistent history (`~/.reedserial_history`, 1000 entries)
- **Tab completion** — Lisp symbols from the lisp-pid API and MAL builtins
- **Syntax highlighting** — parentheses, strings, numbers, known symbols, unbalanced parens
- **Multiline input** — automatically enters multiline mode when parentheses are unbalanced
- **Meta-commands** — `/`-prefixed commands for port management without leaving the shell
- **Auto-detection** — finds the most likely USB-serial port on startup
- **Config file** — persistent port and baud rate via `~/.config/reedserial/config.toml`

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Auto-detect port
reedserial

# Explicit port and baud rate
reedserial --port /dev/ttyUSB0 --baud 115200
reedserial -p /dev/cu.usbserial-0001 -b 9600
```

Any input that does not start with `/` is sent to the connected device. Responses are printed in cyan.

## Meta-commands

| Command | Description |
|---|---|
| `/list-ports` | List available serial ports |
| `/connect` | Auto-detect and connect |
| `/connect <port>` | Connect to port at current baud rate |
| `/connect <port> <baud>` | Connect with explicit baud rate |
| `/disconnect` | Disconnect from current port |
| `/baud <rate>` | Set baud rate (reconnects) |
| `/baud` | Show current baud rate |
| `/help` | Show command help |
| `/quit` | Exit reedserial |

## Config file

`~/.config/reedserial/config.toml`:

```toml
port = "/dev/cu.usbserial-0001"
baud = 115200
```

Priority: CLI args → config file → auto-detect.

## Dependencies

- [reedline](https://github.com/nushell/reedline) — line editor
- [serialport](https://github.com/serialport/serialport-rs) — serial port I/O
- [clap](https://github.com/clap-rs/clap) — CLI argument parsing
- [directories](https://github.com/dirs-dev/directories-rs) — platform config/home paths
