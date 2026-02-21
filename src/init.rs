use std::path::PathBuf;

use directories::BaseDirs;

/// Returns the path to the init file: ~/.config/reedserial/init.lisp
pub fn init_path() -> Option<PathBuf> {
    BaseDirs::new().map(|b| {
        b.home_dir()
            .join(".config")
            .join("reedserial")
            .join("init.lisp")
    })
}

/// Load and parse the init file into a list of top-level Lisp expressions.
///
/// Each expression is returned as a single collapsed line (whitespace normalised).
/// Returns an empty Vec if the file does not exist.
///
/// Parsing rules:
/// - Lines whose first non-whitespace character is `;` are comments — ignored.
/// - Paren/bracket depth is tracked to detect complete s-expressions.
/// - Bare strings, symbols and numbers at depth 0 are also complete expressions.
/// - When an expression is complete, it is collapsed (whitespace normalised) and saved.
pub fn load_init() -> Vec<String> {
    let Some(path) = init_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };

    parse_expressions(&content)
}

fn flush(current: &mut String, expressions: &mut Vec<String>) {
    let collapsed = current.split_whitespace().collect::<Vec<_>>().join(" ");
    if !collapsed.is_empty() {
        expressions.push(collapsed);
    }
    current.clear();
}

pub(crate) fn parse_expressions(content: &str) -> Vec<String> {
    let mut expressions = Vec::new();
    let mut current = String::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for line in content.lines() {
        // Skip pure comment lines when not mid-expression
        if depth == 0 && !in_string && line.trim_start().starts_with(';') {
            continue;
        }

        for ch in line.chars() {
            if escape_next {
                escape_next = false;
                current.push(ch);
                continue;
            }

            if in_string {
                current.push(ch);
                match ch {
                    '\\' => escape_next = true,
                    '"' => {
                        in_string = false;
                        // A string at depth 0 is a complete expression
                        if depth == 0 {
                            flush(&mut current, &mut expressions);
                        }
                    }
                    _ => {}
                }
                continue;
            }

            // Outside a string
            match ch {
                ';' => break, // rest of line is a comment
                '"' => {
                    // Flush any preceding bare atom before the string
                    if depth == 0 && !current.trim().is_empty() {
                        flush(&mut current, &mut expressions);
                    }
                    in_string = true;
                    current.push(ch);
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' | ']' | '}' => {
                    depth -= 1;
                    current.push(ch);
                    if depth <= 0 {
                        depth = 0;
                        flush(&mut current, &mut expressions);
                    }
                }
                ' ' | '\t' if depth == 0 => {
                    // Whitespace between top-level expressions: flush bare atom if any
                    if !current.trim().is_empty() {
                        flush(&mut current, &mut expressions);
                    }
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        // End of line: if depth > 0 we are mid-expression, add a space;
        // if depth == 0 and there is accumulated content it's a bare atom — flush it.
        if depth > 0 || in_string {
            current.push(' ');
        } else if !current.trim().is_empty() {
            flush(&mut current, &mut expressions);
        }
    }

    expressions
}

#[cfg(test)]
mod tests {
    use super::parse_expressions;

    fn parse(input: &str) -> Vec<String> {
        parse_expressions(input)
    }

    #[test]
    fn test_single_line() {
        let exprs = parse("(motor/enable 0 true)");
        assert_eq!(exprs, vec!["(motor/enable 0 true)"]);
    }

    #[test]
    fn test_multiline_expression() {
        let exprs = parse("(let* [x 1]\n  (motor/set-speed 0 x))");
        assert_eq!(exprs, vec!["(let* [x 1] (motor/set-speed 0 x))"]);
    }

    #[test]
    fn test_multiple_expressions() {
        let exprs = parse("(motor/stop-all)\n(motor/enable 0 true)");
        assert_eq!(exprs, vec!["(motor/stop-all)", "(motor/enable 0 true)"]);
    }

    #[test]
    fn test_comment_lines_skipped() {
        let exprs = parse("; setup\n(motor/stop-all)\n; done");
        assert_eq!(exprs, vec!["(motor/stop-all)"]);
    }

    #[test]
    fn test_inline_comment() {
        let exprs = parse("(motor/stop-all) ; safety first");
        assert_eq!(exprs, vec!["(motor/stop-all)"]);
    }

    #[test]
    fn test_paren_in_string() {
        let exprs = parse(r#"(str "hello (world)")"#);
        assert_eq!(exprs, vec![r#"(str "hello (world)")"#]);
    }

    #[test]
    fn test_bare_string() {
        let exprs = parse(r#""Hola Borinot""#);
        assert_eq!(exprs, vec![r#""Hola Borinot""#]);
    }

    #[test]
    fn test_bare_number() {
        let exprs = parse("42");
        assert_eq!(exprs, vec!["42"]);
    }

    #[test]
    fn test_bare_symbol() {
        let exprs = parse("nil");
        assert_eq!(exprs, vec!["nil"]);
    }

    #[test]
    fn test_mixed_bare_and_sexpr() {
        let exprs = parse("\"hello\"\n(motor/stop-all)");
        assert_eq!(exprs, vec!["\"hello\"", "(motor/stop-all)"]);
    }
}
