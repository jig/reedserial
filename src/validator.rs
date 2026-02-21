use reedline::{ValidationResult, Validator};

/// Validates Lisp expressions for multiline editing.
///
/// Returns `Incomplete` when there are unclosed parentheses, causing
/// reedline to enter multiline mode and wait for more input.
pub struct LispValidator;

impl LispValidator {
    pub fn new() -> Self {
        Self
    }
}

impl Validator for LispValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let balance = paren_balance(line);
        if balance > 0 {
            // More opens than closes - need more input
            ValidationResult::Incomplete
        } else {
            // Balanced or extra closes (will be an eval error, not our problem)
            ValidationResult::Complete
        }
    }
}

/// Count net open parentheses (positive = unclosed opens).
/// Ignores parens inside strings and comments.
pub fn paren_balance(input: &str) -> i32 {
    let mut balance: i32 = 0;
    let mut in_string = false;
    let mut in_comment = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
            }
            continue;
        }
        if in_string {
            if ch == '\\' {
                chars.next(); // skip escaped character
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            ';' => in_comment = true,
            '(' => balance += 1,
            ')' => balance -= 1,
            _ => {}
        }
    }

    balance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balanced() {
        assert_eq!(paren_balance("(+ 1 2)"), 0);
        assert_eq!(paren_balance("(def! x (fn* [a] (+ a 1)))"), 0);
    }

    #[test]
    fn test_unbalanced_open() {
        assert_eq!(paren_balance("(def! x"), 1);
        assert_eq!(paren_balance("(let* [a (+ 1"), 2);
    }

    #[test]
    fn test_string_ignored() {
        assert_eq!(paren_balance(r#"(println "(hello)")"#), 0);
    }

    #[test]
    fn test_comment_ignored() {
        assert_eq!(paren_balance("(+ 1 2) ; (unclosed"), 0);
    }
}
