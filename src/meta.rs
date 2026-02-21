use crate::serial::{list_ports, SerialConnection};

/// Result of processing a meta-command
pub enum MetaResult {
    /// Command handled successfully, optional message to display
    Ok(Option<String>),
    /// Request to connect to a port
    Connect { port: String, baud: u32 },
    /// Request to disconnect
    Disconnect,
    /// Request to exit the shell
    Quit,
    /// Unknown or malformed meta-command
    Unknown(String),
}

/// Parse and execute a `/`-prefixed meta-command.
///
/// `line` should include the leading `/`.
/// `current_baud` is the currently configured baud rate.
pub fn handle_meta(line: &str, current_baud: u32) -> MetaResult {
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
    let cmd = parts[0];

    match cmd {
        "/help" | "/?" => MetaResult::Ok(Some(meta_help())),

        "/list-ports" | "/list" | "/ports" => {
            let ports = list_ports();
            if ports.is_empty() {
                MetaResult::Ok(Some("No serial ports found.".to_string()))
            } else {
                let list = ports.join("\n  ");
                MetaResult::Ok(Some(format!("Available ports:\n  {}", list)))
            }
        }

        "/connect" => {
            match parts.len() {
                1 => {
                    // Auto-detect
                    match crate::serial::auto_detect_port() {
                        Some(port) => MetaResult::Connect {
                            port,
                            baud: current_baud,
                        },
                        None => MetaResult::Ok(Some(
                            "No serial ports detected. Use /connect <port> [baud]".to_string(),
                        )),
                    }
                }
                2 => MetaResult::Connect {
                    port: parts[1].to_string(),
                    baud: current_baud,
                },
                _ => {
                    let baud = match parts[2].parse::<u32>() {
                        Ok(b) => b,
                        Err(_) => {
                            return MetaResult::Ok(Some(format!("Invalid baud rate: {}", parts[2])))
                        }
                    };
                    MetaResult::Connect {
                        port: parts[1].to_string(),
                        baud,
                    }
                }
            }
        }

        "/disconnect" => MetaResult::Disconnect,

        "/baud" => {
            if parts.len() < 2 {
                return MetaResult::Ok(Some(format!(
                    "Current baud rate: {} (use /baud <rate> to change)",
                    current_baud
                )));
            }
            match parts[1].parse::<u32>() {
                Ok(baud) => MetaResult::Connect {
                    // Reconnect with new baud (main handles port name)
                    port: "__rebaud__".to_string(),
                    baud,
                },
                Err(_) => MetaResult::Ok(Some(format!("Invalid baud rate: {}", parts[1]))),
            }
        }

        "/quit" | "/exit" | "/q" => MetaResult::Quit,

        _ => MetaResult::Unknown(format!("Unknown command: {}. Try /help", cmd)),
    }
}

/// Check whether a line is a meta-command (starts with /)
pub fn is_meta(line: &str) -> bool {
    line.trim_start().starts_with('/')
}

/// Display a connection status summary
pub fn connection_status(conn: Option<&SerialConnection>) -> String {
    match conn {
        Some(c) => format!("Connected: {} @ {} baud", c.port_name(), c.baud_rate()),
        None => "Disconnected".to_string(),
    }
}

fn meta_help() -> String {
    r#"Meta-commands (prefix /):
  /list-ports            List available serial ports
  /connect               Auto-detect and connect
  /connect <port>        Connect to specified port at current baud
  /connect <port> <baud> Connect with explicit baud rate
  /disconnect            Disconnect from current port
  /baud <rate>           Show or set baud rate
  /help                  Show this help
  /quit                  Exit reedserial

Serial port examples:
  /connect /dev/ttyUSB0 115200
  /connect /dev/cu.usbserial-0001
  /baud 9600"#
        .to_string()
}
