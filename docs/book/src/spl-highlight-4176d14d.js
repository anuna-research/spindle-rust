// Highlight.js language definition for SPL (Spindle Lisp)
hljs.registerLanguage("spl", function(hljs) {
  var SPL_KEYWORDS = {
    keyword: "given normally always except prefer meta import provide claims and not or during must may forbidden",
    literal: "true false inf -inf _"
  };

  var SPL_COMMENT = hljs.COMMENT(";", "$");

  var SPL_STRING = {
    className: "string",
    begin: '"',
    end: '"',
    contains: [hljs.BACKSLASH_ESCAPE]
  };

  var SPL_NUMBER = {
    className: "number",
    variants: [
      { begin: "-?\\d+\\.\\d+[eE][+-]?\\d+" },  // float with exponent
      { begin: "-?\\d+[eE][+-]?\\d+" },           // integer with exponent
      { begin: "-?\\d+\\.\\d+" },                  // decimal
      { begin: "-?\\d+" }                          // integer
    ],
    relevance: 0
  };

  var SPL_VARIABLE = {
    className: "variable",
    begin: "\\?[a-zA-Z_][a-zA-Z0-9_-]*",
    relevance: 5
  };

  var SPL_LABEL = {
    className: "title",
    begin: "(?<=\\((?:normally|always|except|prefer|meta)\\s+)[a-zA-Z_][a-zA-Z0-9_-]*",
    relevance: 0
  };

  var SPL_META_KEY = {
    className: "attr",
    begin: ":[a-zA-Z_][a-zA-Z0-9_-]*",
    relevance: 0
  };

  var SPL_NEGATION = {
    className: "keyword",
    begin: "~",
    relevance: 0
  };

  return {
    name: "SPL",
    aliases: ["spl", "spindle"],
    keywords: SPL_KEYWORDS,
    contains: [
      SPL_COMMENT,
      SPL_STRING,
      SPL_NUMBER,
      SPL_VARIABLE,
      SPL_META_KEY,
      SPL_NEGATION,
      {
        className: "keyword",
        begin: "\\(",
        end: "\\)",
        endsWithParent: false,
        keywords: SPL_KEYWORDS,
        contains: [
          SPL_COMMENT,
          SPL_STRING,
          SPL_NUMBER,
          SPL_VARIABLE,
          SPL_META_KEY,
          SPL_NEGATION,
          "self"
        ]
      }
    ]
  };
});

// Re-highlight any blocks tagged as "lisp" with SPL
document.addEventListener("DOMContentLoaded", function() {
  document.querySelectorAll("pre code.language-spl").forEach(function(block) {
    hljs.highlightElement(block);
  });
});
