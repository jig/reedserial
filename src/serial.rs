use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

/// Result of a serial read operation
#[derive(Debug)]
#[allow(dead_code)]
pub enum ReadResult {
    /// A complete line was received
    Line(String),
    /// Read timed out with no data
    Timeout,
    /// Port was disconnected or errored
    Disconnected(String),
}

/// A serial port connection with buffered line reading
pub struct SerialConnection {
    writer: Box<dyn serialport::SerialPort>,
    reader: BufReader<Box<dyn serialport::SerialPort>>,
    port_name: String,
    baud_rate: u32,
}

impl SerialConnection {
    /// Open a serial port connection
    pub fn open(port_name: &str, baud_rate: u32) -> Result<Self, String> {
        let writer = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| format!("Failed to open {}: {}", port_name, e))?;

        let reader = writer
            .try_clone()
            .map_err(|e| format!("Failed to clone port for reading: {}", e))?;

        Ok(Self {
            writer,
            reader: BufReader::new(reader),
            port_name: port_name.to_string(),
            baud_rate,
        })
    }

    /// Send a line to the serial port (appends \n)
    pub fn send_line(&mut self, line: &str) -> Result<(), String> {
        let data = format!("{}\n", line);
        self.writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        self.writer
            .flush()
            .map_err(|e| format!("Flush error: {}", e))?;
        Ok(())
    }

    /// Read a line from the serial port with timeout
    pub fn read_line(&mut self) -> ReadResult {
        let mut buf = String::new();
        match self.reader.read_line(&mut buf) {
            Ok(0) => ReadResult::Disconnected("Port closed (EOF)".to_string()),
            Ok(_) => ReadResult::Line(buf.trim_end_matches(['\n', '\r']).to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => ReadResult::Timeout,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => ReadResult::Timeout,
            Err(e) => ReadResult::Disconnected(format!("Read error: {}", e)),
        }
    }

    /// Read the response to a command.
    ///
    /// Phase 1 – wait for the first line. The MCU may take arbitrarily long
    ///           (e.g. `(time/sleep-ms 5000)`), so we block until data arrives
    ///           with no upper time limit.
    /// Phase 2 – once the first line is received, collect any additional lines
    ///           that arrive within a short silence window (200 ms).  This
    ///           handles multi-line responses without waiting forever.
    ///
    /// Returns an empty Vec only if the port disconnects before any data.
    pub fn read_response(&mut self) -> Vec<String> {
        let mut lines = Vec::new();

        // Phase 1: wait for first line (no deadline)
        loop {
            match self.read_line() {
                ReadResult::Line(line) => {
                    lines.push(line);
                    break;
                }
                ReadResult::Timeout => continue, // keep waiting
                ReadResult::Disconnected(_) => return lines,
            }
        }

        // Phase 2: collect any further lines within a 200 ms silence window
        let silence = Duration::from_millis(200);
        let mut deadline = std::time::Instant::now() + silence;
        loop {
            if std::time::Instant::now() > deadline {
                break;
            }
            match self.read_line() {
                ReadResult::Line(line) => {
                    lines.push(line);
                    deadline = std::time::Instant::now() + silence; // reset window
                }
                ReadResult::Timeout => break,
                ReadResult::Disconnected(_) => break,
            }
        }

        lines
    }

    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }
}

/// List available serial ports on the system
pub fn list_ports() -> Vec<String> {
    match serialport::available_ports() {
        Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
        Err(_) => Vec::new(),
    }
}

/// Auto-detect the most likely serial port for a microcontroller
pub fn auto_detect_port() -> Option<String> {
    let ports = list_ports();
    // Prefer common USB-serial adapters
    let patterns = [
        "ttyUSB",
        "ttyACM",
        "cu.usbserial",
        "cu.SLAB",
        "cu.usbmodem",
        "cu.wchusbserial",
    ];
    for pattern in &patterns {
        if let Some(p) = ports.iter().find(|p| p.contains(pattern)) {
            return Some(p.clone());
        }
    }
    // Fall back to first available port
    ports.into_iter().next()
}
