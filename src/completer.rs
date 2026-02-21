use reedline::{Completer, Span, Suggestion};

/// Lisp symbols available for tab completion.
/// Includes the full lisp-pid API plus common Lisp builtins.
const LISP_SYMBOLS: &[&str] = &[
    // lisp-pid motor API
    "motor/set-speed",
    "motor/set-pid",
    "motor/enable",
    "motor/set-direction-reversed",
    "motor/get-state",
    "motor/set-frequency",
    "motor/set-timeout-ms",
    "motor/stop-all",
    "motor/emergency-stop",
    // lisp-pid servo API
    "servo/set-angle",
    // lisp-pid system API
    "system/battery-voltage",
    // MAL / Lisp builtins
    "def!",
    "let*",
    "fn*",
    "if",
    "do",
    "quote",
    "quasiquote",
    "unquote",
    "splice-unquote",
    "defmacro!",
    "macroexpand",
    "try*",
    "catch*",
    "nil",
    "true",
    "false",
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
    "vector?",
    "hash-map",
    "map?",
    "assoc",
    "dissoc",
    "get",
    "contains?",
    "keys",
    "vals",
    "atom",
    "atom?",
    "deref",
    "reset!",
    "swap!",
    "symbol",
    "symbol?",
    "keyword",
    "keyword?",
    "number?",
    "string?",
    "nil?",
    "true?",
    "false?",
    "fn?",
    "macro?",
    "sequential?",
    "throw",
    "read-string",
    "eval",
    "load-file",
    "time-ms",
    "time/sleep-ms",
    "time/sleep-us",
    "conj",
    "seq",
    "with-meta",
    "meta",
    "gensym",
    "readline",
];

pub struct LispCompleter {
    symbols: Vec<String>,
}

impl LispCompleter {
    pub fn new() -> Self {
        let mut symbols: Vec<String> = LISP_SYMBOLS.iter().map(|s| s.to_string()).collect();
        symbols.sort();
        Self { symbols }
    }

    /// Add a custom symbol to the completer (e.g. user-defined functions)
    #[allow(dead_code)]
    pub fn add_symbol(&mut self, sym: &str) {
        if !self.symbols.contains(&sym.to_string()) {
            self.symbols.push(sym.to_string());
            self.symbols.sort();
        }
    }
}

impl Completer for LispCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        // Find the start of the current token (symbol being typed)
        let token_start = line[..pos]
            .rfind(|c: char| c == '(' || c == ' ' || c == '\t' || c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        let prefix = &line[token_start..pos];

        if prefix.is_empty() {
            return Vec::new();
        }

        self.symbols
            .iter()
            .filter(|sym| sym.starts_with(prefix))
            .map(|sym| Suggestion {
                value: sym.clone(),
                description: None,
                style: None,
                extra: None,
                span: Span::new(token_start, pos),
                append_whitespace: false,
            })
            .collect()
    }
}
