use std::collections::HashSet;

use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};

/// Lisp syntax highlighter for reedline.
///
/// Coloring scheme:
/// - Parentheses: cyan
/// - Strings: green
/// - Numbers: yellow
/// - Known symbols / builtins: bold white
/// - Unbalanced parens: red underline
/// - Comments (;...): dark gray
pub struct LispHighlighter {
    known_symbols: HashSet<String>,
}

impl LispHighlighter {
    /// Create a highlighter with the hardcoded fallback symbol set.
    /// Used when disconnected.
    pub fn new() -> Self {
        let known_symbols = FALLBACK_SYMBOLS.iter().map(|s| s.to_string()).collect();
        Self { known_symbols }
    }

    /// Create a highlighter from a live symbol list fetched from the MCU.
    pub fn with_symbols(symbols: Vec<String>) -> Self {
        Self {
            known_symbols: symbols.into_iter().collect(),
        }
    }
}

impl Highlighter for LispHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled = StyledText::new();

        let paren_depth_ok = {
            let mut depth: i32 = 0;
            let mut ok = true;
            for ch in line.chars() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth < 0 {
                            ok = false;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            ok && depth == 0
        };

        let paren_style = if paren_depth_ok || line.is_empty() {
            Style::new().fg(Color::Cyan)
        } else {
            Style::new().fg(Color::Red).underline()
        };

        let mut chars = line.chars().peekable();
        let mut current = String::new();
        let mut in_string = false;
        let mut in_comment = false;

        while let Some(ch) = chars.next() {
            if in_comment {
                current.push(ch);
                if ch == '\n' {
                    styled.push((Style::new().fg(Color::DarkGray), current.clone()));
                    current.clear();
                    in_comment = false;
                }
                continue;
            }

            if in_string {
                current.push(ch);
                if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else if ch == '"' {
                    styled.push((Style::new().fg(Color::Green), current.clone()));
                    current.clear();
                    in_string = false;
                }
                continue;
            }

            match ch {
                ';' => {
                    // Flush any pending token first
                    if !current.is_empty() {
                        styled.push((token_style(&current, &self.known_symbols), current.clone()));
                        current.clear();
                    }
                    current.push(ch);
                    in_comment = true;
                }
                '"' => {
                    if !current.is_empty() {
                        styled.push((token_style(&current, &self.known_symbols), current.clone()));
                        current.clear();
                    }
                    current.push(ch);
                    in_string = true;
                }
                '(' | ')' => {
                    if !current.is_empty() {
                        styled.push((token_style(&current, &self.known_symbols), current.clone()));
                        current.clear();
                    }
                    styled.push((paren_style, ch.to_string()));
                }
                ' ' | '\t' | '\n' => {
                    if !current.is_empty() {
                        styled.push((token_style(&current, &self.known_symbols), current.clone()));
                        current.clear();
                    }
                    styled.push((Style::new(), ch.to_string()));
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        // Flush remaining
        if !current.is_empty() {
            if in_comment {
                styled.push((Style::new().fg(Color::DarkGray), current));
            } else if in_string {
                // Unclosed string
                styled.push((Style::new().fg(Color::Green), current));
            } else {
                styled.push((token_style(&current, &self.known_symbols), current));
            }
        }

        styled
    }
}

/// Determine the style for a non-paren, non-string token.
fn token_style(token: &str, known_symbols: &HashSet<String>) -> Style {
    // Number literal
    if token.parse::<f64>().is_ok() {
        return Style::new().fg(Color::Yellow);
    }
    // Special literals
    if matches!(token, "nil" | "true" | "false") {
        return Style::new().fg(Color::Purple);
    }
    // Special forms / builtins
    if known_symbols.contains(token) {
        return Style::new().bold();
    }
    // Regular symbol or unknown
    Style::new().fg(Color::White)
}

/// Fallback symbol list used when disconnected.
const FALLBACK_SYMBOLS: &[&str] = &[
    "def!",
    "let*",
    "fn*",
    "if",
    "do",
    "quote",
    "quasiquote",
    "defmacro!",
    "macroexpand",
    "try*",
    "catch*",
    "not",
    "and",
    "or",
    "println",
    "prn",
    "pr-str",
    "str",
    "list",
    "list?",
    "empty?",
    "count",
    "cons",
    "concat",
    "nth",
    "first",
    "rest",
    "map",
    "apply",
    "filter",
    "reduce",
    "vec",
    "vector",
    "hash-map",
    "assoc",
    "dissoc",
    "get",
    "contains?",
    "keys",
    "vals",
    "atom",
    "deref",
    "reset!",
    "swap!",
    "eval",
    "time/sleep-ms",
    "time/sleep-us",
    "motor/set-speed",
    "motor/set-pid",
    "motor/enable",
    "motor/set-direction-reversed",
    "motor/get-state",
    "motor/set-frequency",
    "motor/set-timeout-ms",
    "motor/stop-all",
    "motor/emergency-stop",
    "servo/set-angle",
    "system/battery-voltage",
];
