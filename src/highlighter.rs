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
pub struct LispHighlighter;

impl LispHighlighter {
    pub fn new() -> Self {
        Self
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
                        styled.push((token_style(&current), current.clone()));
                        current.clear();
                    }
                    current.push(ch);
                    in_comment = true;
                }
                '"' => {
                    if !current.is_empty() {
                        styled.push((token_style(&current), current.clone()));
                        current.clear();
                    }
                    current.push(ch);
                    in_string = true;
                }
                '(' | ')' => {
                    if !current.is_empty() {
                        styled.push((token_style(&current), current.clone()));
                        current.clear();
                    }
                    styled.push((paren_style, ch.to_string()));
                }
                ' ' | '\t' | '\n' => {
                    if !current.is_empty() {
                        styled.push((token_style(&current), current.clone()));
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
                styled.push((token_style(&current), current));
            }
        }

        styled
    }
}

/// Determine the style for a non-paren, non-string token.
fn token_style(token: &str) -> Style {
    // Number literal
    if token.parse::<f64>().is_ok() {
        return Style::new().fg(Color::Yellow);
    }
    // Special literals
    if matches!(token, "nil" | "true" | "false") {
        return Style::new().fg(Color::Purple);
    }
    // Special forms / builtins
    if is_known_symbol(token) {
        return Style::new().bold();
    }
    // Regular symbol or unknown
    Style::new().fg(Color::White)
}

fn is_known_symbol(token: &str) -> bool {
    matches!(
        token,
        "def!"
            | "let*"
            | "fn*"
            | "if"
            | "do"
            | "quote"
            | "quasiquote"
            | "defmacro!"
            | "macroexpand"
            | "try*"
            | "catch*"
            | "not"
            | "and"
            | "or"
            | "println"
            | "prn"
            | "pr-str"
            | "str"
            | "list"
            | "list?"
            | "empty?"
            | "count"
            | "cons"
            | "concat"
            | "nth"
            | "first"
            | "rest"
            | "map"
            | "apply"
            | "filter"
            | "reduce"
            | "vec"
            | "vector"
            | "hash-map"
            | "assoc"
            | "dissoc"
            | "get"
            | "contains?"
            | "keys"
            | "vals"
            | "atom"
            | "deref"
            | "reset!"
            | "swap!"
            | "eval"
            | "time-ms"
            | "time/sleep-ms"
            | "time/sleep-us"
            | "motor/set-speed"
            | "motor/set-pid"
            | "motor/enable"
            | "motor/set-direction-reversed"
            | "motor/get-state"
            | "motor/set-frequency"
            | "motor/set-timeout-ms"
            | "motor/stop-all"
            | "motor/emergency-stop"
            | "servo/set-angle"
            | "system/battery-voltage"
    )
}
