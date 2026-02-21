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
    // lisp-pid time API (sleep)
    "time/sleep-ns",
    "time/sleep-us",
    "time/sleep-ms",
    "time/sleep-s",
    // lisp-pid time API (query)
    "time/ns",
    "time/us",
    "time/ms",
    "time/s",
    // MAL special forms
    "def!",
    "defmacro!",
    "let*",
    "fn*",
    "if",
    "do",
    "quote",
    "quasiquote",
    "unquote",
    "splice-unquote",
    "macroexpand",
    "try*",
    "catch*",
    // MAL core functions
    "apply",
    "assoc",
    "atom",
    "atom?",
    "concat",
    "conj",
    "cons",
    "contains?",
    "count",
    "deref",
    "dissoc",
    "empty?",
    "eval",
    "false?",
    "first",
    "fn?",
    "gensym",
    "get",
    "hash-map",
    "int",
    "int?",
    "float",
    "float?",
    "keys",
    "keyword",
    "keyword?",
    "list",
    "list?",
    "load-file",
    "macro?",
    "map",
    "map?",
    "meta",
    "meta?",
    "nil?",
    "not",
    "nth",
    "number?",
    "pr-str",
    "prn",
    "println",
    "read-string",
    "readline",
    "reset!",
    "rest",
    "seq",
    "sequential?",
    "str",
    "string?",
    "swap!",
    "symbol",
    "symbol?",
    "throw",
    "true?",
    "vals",
    "vec",
    "vector",
    "vector?",
    "with-meta",
    // MAL constants
    "nil",
    "true",
    "false",
    // MAL arithmetic / comparison operators
    "+",
    "-",
    "*",
    "/",
    "<",
    "<=",
    "=",
    ">",
    ">=",
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
