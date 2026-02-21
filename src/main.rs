mod completer;
mod highlighter;
mod init;
mod meta;
mod serial;
mod validator;

use std::path::PathBuf;

use clap::Parser;
use directories::BaseDirs;
use nu_ansi_term::Color;
use reedline::{
    default_emacs_keybindings, ColumnarMenu, Emacs, FileBackedHistory, KeyCode, KeyModifiers,
    MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus, Reedline,
    ReedlineEvent, ReedlineMenu, Signal,
};
use serde::Deserialize;

use completer::LispCompleter;
use highlighter::LispHighlighter;
use init::load_init;
use meta::{connection_status, handle_meta, is_meta, MetaResult};
use serial::{auto_detect_port, SerialConnection};
use validator::LispValidator;

// ─── CLI Arguments ───────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "reedserial", about = "Lisp REPL shell over serial port")]
struct Args {
    /// Serial port device (e.g. /dev/ttyUSB0, /dev/cu.usbserial-0001)
    #[arg(short, long)]
    port: Option<String>,

    /// Baud rate [default: 115200]
    #[arg(short, long)]
    baud: Option<u32>,
}

// ─── Config File ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct Config {
    port: Option<String>,
    baud: Option<u32>,
}

fn config_path() -> Option<PathBuf> {
    BaseDirs::new().map(|b| {
        b.home_dir()
            .join(".config")
            .join("reedserial")
            .join("config.toml")
    })
}

fn load_config() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Config::default();
    };
    toml::from_str(&content).unwrap_or_default()
}

// ─── Prompt ──────────────────────────────────────────────────────────────────

struct LispPrompt {
    port: String,
    pub ps1: Option<String>,
    pub ps2: Option<String>,
}

impl LispPrompt {
    fn new_with_prompts(port: &str, ps1: Option<String>, ps2: Option<String>) -> Self {
        Self {
            port: port.to_string(),
            ps1,
            ps2,
        }
    }

    fn disconnected() -> Self {
        Self {
            port: String::new(),
            ps1: None,
            ps2: None,
        }
    }
}

impl Prompt for LispPrompt {
    fn render_prompt_left(&self) -> std::borrow::Cow<'_, str> {
        if let Some(ps1) = &self.ps1 {
            return std::borrow::Cow::Borrowed(ps1.as_str());
        }
        if self.port.is_empty() {
            std::borrow::Cow::Borrowed("(disconnected) » ")
        } else {
            let short = self.port.split('/').last().unwrap_or(&self.port);
            std::borrow::Cow::Owned(format!("({}) » ", short))
        }
    }

    fn render_prompt_right(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> std::borrow::Cow<'_, str> {
        if let Some(ps2) = &self.ps2 {
            return std::borrow::Cow::Borrowed(ps2.as_str());
        }
        std::borrow::Cow::Borrowed("  .. ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> std::borrow::Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        std::borrow::Cow::Owned(format!(
            "({}reverse-search: {}) » ",
            prefix, history_search.term
        ))
    }
}

// ─── History Path ─────────────────────────────────────────────────────────────

fn history_path() -> PathBuf {
    BaseDirs::new()
        .map(|b| b.home_dir().join(".reedserial_history"))
        .unwrap_or_else(|| PathBuf::from(".reedserial_history"))
}

// ─── Build Reedline Editor ───────────────────────────────────────────────────

/// Fetch the live symbol list from the MCU by calling `(env/keys)`.
/// Returns None if not connected or if the response cannot be parsed.
fn fetch_symbols(conn: &mut SerialConnection) -> Option<Vec<String>> {
    conn.send_line("(env/keys)").ok()?;
    let lines = conn.read_response();
    let line = lines.first()?;
    // Response is a MAL list: (sym1 sym2 sym3 ...)
    let trimmed = line.trim().trim_start_matches('(').trim_end_matches(')');
    let symbols: Vec<String> = trimmed.split_whitespace().map(|s| s.to_string()).collect();
    if symbols.is_empty() {
        None
    } else {
        Some(symbols)
    }
}

/// Query a single prompt variable (*PS1* or *PS2*) from the MCU.
/// Returns (value, is_fn): the resolved string and whether the var is a function.
/// Returns None if the symbol is not defined or evaluation fails.
fn fetch_prompt_var(conn: &mut SerialConnection, var: &str) -> Option<(String, bool)> {
    // First check if it's a function
    let is_fn = {
        let expr = format!("(fn? {var})");
        conn.send_line(&expr).ok()?;
        let lines = conn.read_response();
        lines.first().map(|l| l.trim() == "true").unwrap_or(false)
    };
    // Now evaluate: call it if function, stringify if value
    let expr = if is_fn {
        format!("({var})")
    } else {
        format!("(str {var})")
    };
    conn.send_line(&expr).ok()?;
    let lines = conn.read_response();
    let line = lines.first()?;
    if line.starts_with("Error") || line.starts_with("ERROR") {
        return None;
    }
    let s = line.trim().trim_matches('"').to_string();
    if s.is_empty() {
        None
    } else {
        Some((s, is_fn))
    }
}

/// Re-evaluate a prompt function variable (*PS1* or *PS2*) on the MCU.
/// Only called when we know the var is a function.
fn eval_prompt_fn(conn: &mut SerialConnection, var: &str) -> Option<String> {
    let expr = format!("({var})");
    conn.send_line(&expr).ok()?;
    let lines = conn.read_response();
    let line = lines.first()?;
    if line.starts_with("Error") || line.starts_with("ERROR") {
        return None;
    }
    let s = line.trim().trim_matches('"').to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Fetch *PS1* and *PS2* prompt overrides from the MCU.
/// Returns ((ps1_value, ps1_is_fn), (ps2_value, ps2_is_fn)).
fn fetch_prompt_vars(
    conn: &mut SerialConnection,
) -> (Option<(String, bool)>, Option<(String, bool)>) {
    let ps1 = fetch_prompt_var(conn, "*PS1*");
    let ps2 = fetch_prompt_var(conn, "*PS2*");
    (ps1, ps2)
}

fn build_editor(
    port_name: &str,
    symbols: Option<Vec<String>>,
    ps1: Option<String>,
    ps2: Option<String>,
) -> (Reedline, LispPrompt) {
    let history = Box::new(
        FileBackedHistory::with_file(1000, history_path()).expect("Failed to open history file"),
    );

    let completer = Box::new(match &symbols {
        Some(syms) => LispCompleter::with_symbols(syms.clone()),
        None => LispCompleter::new(),
    });
    let highlighter = Box::new(match symbols {
        Some(syms) => LispHighlighter::with_symbols(syms),
        None => LispHighlighter::new(),
    });
    let validator = Box::new(LispValidator::new());

    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::CONTROL,
        KeyCode::Char('d'),
        ReedlineEvent::CtrlD,
    );

    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
    let edit_mode = Box::new(Emacs::new(keybindings));

    let editor = Reedline::create()
        .with_history(history)
        .with_completer(completer)
        .with_highlighter(highlighter)
        .with_validator(validator)
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_edit_mode(edit_mode);

    let prompt = if port_name.is_empty() {
        LispPrompt::disconnected()
    } else {
        LispPrompt::new_with_prompts(port_name, ps1, ps2)
    };

    (editor, prompt)
}

// ─── Init Script ──────────────────────────────────────────────────────────────

/// Run ~/.config/reedserial/init.lisp against an open connection.
///
/// Each top-level expression is printed in dark gray, sent, and its response
/// printed in cyan. Aborts on the first error response from the MCU.
fn run_init(conn: &mut SerialConnection) {
    let expressions = load_init();
    if expressions.is_empty() {
        return;
    }

    println!("{}", Color::DarkGray.paint("Running init.lisp..."));

    for expr in &expressions {
        println!("{}", Color::DarkGray.paint(format!("  {}", expr)));

        if let Err(e) = conn.send_line(expr) {
            eprintln!("{}", Color::Red.paint(format!("Init send error: {}", e)));
            return;
        }

        let lines = conn.read_response();
        for line in &lines {
            println!("  {}", Color::Cyan.paint(line));
            if line.starts_with("Error") || line.starts_with("ERROR") {
                eprintln!(
                    "{}",
                    Color::Red.paint("Init aborted: error response from MCU.")
                );
                return;
            }
        }
    }

    println!("{}", Color::DarkGray.paint("Init done."));
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();
    let config = load_config();

    // Priority: CLI arg → config file → 115200
    let initial_port = args.port.or(config.port).or_else(auto_detect_port);
    let baud = args.baud.or(config.baud).unwrap_or(115_200);

    let mut connection: Option<SerialConnection> = None;
    let mut current_baud = baud;
    let mut current_port = String::new();

    if let Some(ref port) = initial_port {
        match SerialConnection::open(port, baud) {
            Ok(conn) => {
                current_port = port.clone();
                current_baud = baud;
                connection = Some(conn);
                println!(
                    "{}",
                    Color::Green.paint(connection_status(connection.as_ref()))
                );
                run_init(connection.as_mut().unwrap());
            }
            Err(e) => {
                eprintln!("{}", Color::Yellow.paint(format!("Warning: {}", e)));
                eprintln!("Use /connect to connect manually.");
            }
        }
    } else {
        println!(
            "{}",
            Color::Yellow.paint("No serial port detected. Use /connect to connect.")
        );
    }

    println!("Type /help for available commands. Ctrl+D or /quit to exit.\n");

    let initial_symbols = connection.as_mut().and_then(fetch_symbols);
    let (raw_ps1, raw_ps2) = connection
        .as_mut()
        .map(fetch_prompt_vars)
        .unwrap_or((None, None));
    let mut ps1_is_fn = raw_ps1.as_ref().map(|(_, f)| *f).unwrap_or(false);
    let mut ps2_is_fn = raw_ps2.as_ref().map(|(_, f)| *f).unwrap_or(false);
    let (mut editor, mut prompt) = build_editor(
        &current_port,
        initial_symbols,
        raw_ps1.map(|(v, _)| v),
        raw_ps2.map(|(v, _)| v),
    );

    loop {
        // Re-evaluate prompt functions before each read_line
        if ps1_is_fn {
            if let Some(conn) = connection.as_mut() {
                prompt.ps1 = eval_prompt_fn(conn, "*PS1*");
            }
        }
        if ps2_is_fn {
            if let Some(conn) = connection.as_mut() {
                prompt.ps2 = eval_prompt_fn(conn, "*PS2*");
            }
        }

        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if is_meta(trimmed) {
                    match handle_meta(trimmed, current_baud) {
                        MetaResult::Ok(Some(msg)) => println!("{}", msg),
                        MetaResult::Ok(None) => {}

                        MetaResult::Connect { port, baud } => {
                            let target_port = if port == "__rebaud__" {
                                if current_port.is_empty() {
                                    eprintln!(
                                        "{}",
                                        Color::Red.paint("Not connected. Use /connect <port>")
                                    );
                                    current_baud = baud;
                                    continue;
                                }
                                current_port.clone()
                            } else {
                                port
                            };

                            connection = None;
                            match SerialConnection::open(&target_port, baud) {
                                Ok(conn) => {
                                    current_port = target_port;
                                    current_baud = baud;
                                    connection = Some(conn);
                                    println!(
                                        "{}",
                                        Color::Green.paint(connection_status(connection.as_ref()))
                                    );
                                    run_init(connection.as_mut().unwrap());
                                    let symbols = fetch_symbols(connection.as_mut().unwrap());
                                    let (raw_ps1, raw_ps2) =
                                        fetch_prompt_vars(connection.as_mut().unwrap());
                                    ps1_is_fn = raw_ps1.as_ref().map(|(_, f)| *f).unwrap_or(false);
                                    ps2_is_fn = raw_ps2.as_ref().map(|(_, f)| *f).unwrap_or(false);
                                    let (new_editor, new_prompt) = build_editor(
                                        &current_port,
                                        symbols,
                                        raw_ps1.map(|(v, _)| v),
                                        raw_ps2.map(|(v, _)| v),
                                    );
                                    editor = new_editor;
                                    prompt = new_prompt;
                                }
                                Err(e) => {
                                    eprintln!("{}", Color::Red.paint(format!("Error: {}", e)));
                                    current_baud = baud;
                                }
                            }
                        }

                        MetaResult::Disconnect => {
                            connection = None;
                            current_port.clear();
                            ps1_is_fn = false;
                            ps2_is_fn = false;
                            println!("{}", Color::Yellow.paint("Disconnected."));
                            let (new_editor, new_prompt) = build_editor("", None, None, None);
                            editor = new_editor;
                            prompt = new_prompt;
                        }

                        MetaResult::Quit => {
                            println!("Bye.");
                            break;
                        }

                        MetaResult::Unknown(msg) => {
                            eprintln!("{}", Color::Red.paint(msg));
                        }
                    }
                    continue;
                }

                // Lisp expression — send to MCU
                match connection.as_mut() {
                    None => {
                        eprintln!(
                            "{}",
                            Color::Red
                                .paint("Not connected. Use /connect <port> [baud] to connect.")
                        );
                    }
                    Some(conn) => {
                        // Collapse multiline input to a single line before sending
                        let single_line = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
                        if let Err(e) = conn.send_line(&single_line) {
                            eprintln!("{}", Color::Red.paint(format!("Send error: {}", e)));
                            eprintln!("Use /connect to reconnect.");
                            connection = None;
                            current_port.clear();
                            ps1_is_fn = false;
                            ps2_is_fn = false;
                            let (new_editor, new_prompt) = build_editor("", None, None, None);
                            editor = new_editor;
                            prompt = new_prompt;
                            continue;
                        }

                        let conn = connection.as_mut().unwrap();
                        let lines = conn.read_response();
                        for response_line in &lines {
                            println!("{}", Color::Cyan.paint(response_line));
                        }
                    }
                }
            }

            Ok(Signal::CtrlC) => {
                // Clear current line, continue
                println!("^C");
            }

            Ok(Signal::CtrlD) => {
                println!("Bye.");
                break;
            }

            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}
