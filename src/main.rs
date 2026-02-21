mod completer;
mod highlighter;
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
}

impl LispPrompt {
    fn new(port: &str) -> Self {
        Self {
            port: port.to_string(),
        }
    }

    fn disconnected() -> Self {
        Self {
            port: String::new(),
        }
    }
}

impl Prompt for LispPrompt {
    fn render_prompt_left(&self) -> std::borrow::Cow<'_, str> {
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

fn build_editor(port_name: &str) -> (Reedline, LispPrompt) {
    let history = Box::new(
        FileBackedHistory::with_file(1000, history_path()).expect("Failed to open history file"),
    );

    let completer = Box::new(LispCompleter::new());
    let highlighter = Box::new(LispHighlighter::new());
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
        LispPrompt::new(port_name)
    };

    (editor, prompt)
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

    let (mut editor, mut prompt) = build_editor(&current_port);

    loop {
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
                                    let (new_editor, new_prompt) = build_editor(&current_port);
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
                            println!("{}", Color::Yellow.paint("Disconnected."));
                            let (new_editor, new_prompt) = build_editor("");
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
                        if let Err(e) = conn.send_line(trimmed) {
                            eprintln!("{}", Color::Red.paint(format!("Send error: {}", e)));
                            eprintln!("Use /connect to reconnect.");
                            connection = None;
                            current_port.clear();
                            let (new_editor, new_prompt) = build_editor("");
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
