// Highlight.js language definition for SPL (Spindle Lisp)
hljs.registerLanguage("spl", function(hljs) {
  // ; line comments
  var COMMENT = hljs.COMMENT(";", "$");

  // "strings"
  var STRING = {
    className: "string",
    begin: '"', end: '"',
    contains: [hljs.BACKSLASH_ESCAPE]
  };

  // Numbers: 42, 3.14, 1.5e2
  var NUMBER = {
    className: "number",
    variants: [
      { begin: "-?\\d+\\.\\d+[eE][+-]?\\d+" },
      { begin: "-?\\d+[eE][+-]?\\d+" },
      { begin: "-?\\d+\\.\\d+" },
      { begin: "-?\\d+" }
    ],
    relevance: 0
  };

  // ?variables
  var VARIABLE = {
    className: "variable",
    begin: "\\?[a-zA-Z_][a-zA-Z0-9_-]*",
    relevance: 5
  };

  // :meta-keys
  var META_KEY = {
    className: "attr",
    begin: ":[a-zA-Z_][a-zA-Z0-9_-]*",
    relevance: 0
  };

  // ~negation operator
  var TILDE = {
    className: "built_in",
    begin: "~",
    relevance: 0
  };

  // Form keywords (purple) — top-level statement forms
  var FORM_KEYWORDS = [
    "given", "normally", "always", "except",
    "prefer", "meta", "import", "provide", "claims",
    "trusts", "decays", "threshold"
  ];

  // Logic keywords (orange via built_in) — operators within rules
  var LOGIC_KEYWORDS = [
    "and", "or", "not", "during",
    "must", "may", "forbidden",
    "bind", "abs", "div", "rem", "min", "max",
    "exponential", "linear", "step"
  ];

  // A form-keyword
  var FORM_OPEN = {
    className: "keyword",
    begin: "\\b(" + FORM_KEYWORDS.join("|") + ")\\b",
    relevance: 2
  };

  // Logic operator keywords highlighted differently
  var LOGIC_OP = {
    className: "built_in",
    begin: "\\b(" + LOGIC_KEYWORDS.join("|") + ")\\b",
    relevance: 0
  };

  // Literals
  var LITERAL = {
    className: "literal",
    begin: "\\b(true|false|inf|-inf|_)\\b",
    relevance: 0
  };

  // Labels for rule forms: (normally LABEL body head)
  // Only match as title when there are 3+ args (label + body + head),
  // i.e. the word is followed by whitespace and at least two more tokens.
  // (normally birds-fly bird flies) -> birds-fly is title
  // (normally bird flies)           -> bird is NOT title (only 2 args)
  var RULE_LABEL = {
    className: "title",
    begin: "(?:\\b(?:normally|always|except)\\s+)([a-zA-Z_][a-zA-Z0-9_-]*)",
    // Lookahead: after the label, need whitespace + token + whitespace + token
    // before closing paren. Token = word or paren-group.
    beginKeywords: "normally always except",
    returnBegin: true,
    contains: [
      {
        className: "keyword",
        begin: "\\b(normally|always|except)\\b"
      },
      {
        // Only highlight as title if followed by 2+ more tokens
        className: "title",
        begin: "[a-zA-Z_][a-zA-Z0-9_-]*(?=\\s+(?:\\(|[a-zA-Z_?~])[\\s\\S]*?\\s+(?:\\(|[a-zA-Z_?~]))",
        relevance: 2
      }
    ]
  };

  // Labels for prefer/meta: first arg is always a label
  // (prefer label1 label2) — all args are labels
  var PREFER_LABELS = {
    begin: "\\bprefer\\s+",
    returnBegin: true,
    contains: [
      {
        className: "keyword",
        begin: "\\bprefer\\b"
      },
      {
        className: "title",
        begin: "[a-zA-Z_][a-zA-Z0-9_-]*",
        relevance: 2
      }
    ]
  };

  // Labels for meta: (meta label ...)
  var META_LABEL = {
    begin: "\\bmeta\\s+",
    returnBegin: true,
    contains: [
      {
        className: "keyword",
        begin: "\\bmeta\\b"
      },
      {
        className: "title",
        begin: "[a-zA-Z_][a-zA-Z0-9_-]*",
        relevance: 2
      }
    ]
  };

  return {
    name: "SPL",
    aliases: ["spl", "spindle"],
    case_insensitive: false,
    contains: [
      COMMENT,
      STRING,
      NUMBER,
      VARIABLE,
      META_KEY,
      TILDE,
      LITERAL,
      RULE_LABEL,
      PREFER_LABELS,
      META_LABEL,
      FORM_OPEN,
      LOGIC_OP
    ]
  };
});

// Highlight SPL blocks immediately (this script loads after highlight.js and book.js)
document.querySelectorAll("pre code.language-spl").forEach(function(block) {
  block.innerHTML = block.textContent;
  block.classList.add("hljs");
  hljs.highlightBlock(block);
});
