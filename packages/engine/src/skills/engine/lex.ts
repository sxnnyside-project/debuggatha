/**
 * A small lexer, not a parser: it finds comments and string literals so that
 * detectors can look at code without reading inside them. `masked` has the same
 * length and line layout as the source, with comment text and string contents
 * replaced by spaces, so a match's column in `masked` is its column in the source.
 */

export interface StringLiteral {
  /** Text between the quotes, unescaped only as far as the source wrote it. */
  value: string;
  /** Offset of the opening quote and just past the closing one. */
  start: number;
  end: number;
}

export interface Comment {
  /** 1-based line of the comment (each line of a block comment is its own entry). */
  line: number;
  text: string;
}

export interface LexResult {
  masked: string;
  strings: StringLiteral[];
  comments: Comment[];
}

interface Syntax {
  lineComments: string[];
  block: [string, string] | undefined;
  quotes: string;
  backtick: boolean;
  tripleQuotes: boolean;
  regexLiterals: boolean;
  /** `'` starts a char literal only when it closes within a few characters (Rust lifetimes are not strings). */
  charLiteralQuote: boolean;
}

const C_LIKE: Syntax = {
  lineComments: ["//"],
  block: ["/*", "*/"],
  quotes: "\"'",
  backtick: false,
  tripleQuotes: false,
  regexLiterals: false,
  charLiteralQuote: false,
};

const SYNTAX_BY_EXTENSION: Record<string, Syntax> = {
  ts: { ...C_LIKE, backtick: true, regexLiterals: true },
  tsx: { ...C_LIKE, backtick: true, regexLiterals: true },
  js: { ...C_LIKE, backtick: true, regexLiterals: true },
  jsx: { ...C_LIKE, backtick: true, regexLiterals: true },
  mjs: { ...C_LIKE, backtick: true, regexLiterals: true },
  cjs: { ...C_LIKE, backtick: true, regexLiterals: true },
  java: C_LIKE,
  cs: C_LIKE,
  swift: C_LIKE,
  scala: C_LIKE,
  kt: { ...C_LIKE, tripleQuotes: true },
  kts: { ...C_LIKE, tripleQuotes: true },
  dart: { ...C_LIKE, tripleQuotes: true },
  go: { ...C_LIKE, quotes: '"', backtick: true },
  rs: { ...C_LIKE, quotes: '"', charLiteralQuote: true },
  php: { ...C_LIKE, lineComments: ["//", "#"] },
  py: {
    lineComments: ["#"],
    block: undefined,
    quotes: "\"'",
    backtick: false,
    tripleQuotes: true,
    regexLiterals: false,
    charLiteralQuote: false,
  },
  rb: {
    lineComments: ["#"],
    block: undefined,
    quotes: "\"'",
    backtick: false,
    tripleQuotes: false,
    regexLiterals: false,
    charLiteralQuote: false,
  },
};

const REGEX_PRECEDERS = new Set([
  "(",
  ",",
  "=",
  ":",
  "[",
  "!",
  "&",
  "|",
  "?",
  "{",
  "}",
  ";",
  "+",
  "-",
  "*",
  "%",
  "<",
  ">",
  "~",
  "^",
]);
const REGEX_KEYWORDS =
  /(?:^|[^\w$])(?:return|typeof|case|in|of|do|else|void|delete|throw|yield|await)$/;

function syntaxFor(file: string): Syntax | undefined {
  const extension = file.slice(file.lastIndexOf(".") + 1).toLowerCase();
  return SYNTAX_BY_EXTENSION[extension];
}

/** Lexes `content`; a language with no known syntax comes back unmasked (nothing is assumed). */
export function lex(content: string, file: string): LexResult {
  const syntax = syntaxFor(file);
  if (!syntax) return { masked: content, strings: [], comments: [] };

  const chars = content.split("");
  const strings: StringLiteral[] = [];
  const comments: Comment[] = [];
  const length = content.length;

  const lineStarts = [0];
  for (let i = 0; i < length; i += 1) if (content[i] === "\n") lineStarts.push(i + 1);
  const lineOf = (offset: number) => {
    let low = 0;
    let high = lineStarts.length - 1;
    while (low < high) {
      const mid = (low + high + 1) >> 1;
      if ((lineStarts[mid] ?? 0) <= offset) low = mid;
      else high = mid - 1;
    }
    return low + 1;
  };

  const blank = (from: number, to: number) => {
    for (let i = from; i < to; i += 1) {
      if (chars[i] !== "\n" && chars[i] !== "\r") chars[i] = " ";
    }
  };

  let lastSignificant = "";
  let i = 0;
  while (i < length) {
    const ch = content[i] as string;

    const lineComment = syntax.lineComments.find((marker) => content.startsWith(marker, i));
    if (lineComment) {
      let end = content.indexOf("\n", i);
      if (end === -1) end = length;
      comments.push({ line: lineOf(i), text: content.slice(i + lineComment.length, end) });
      blank(i, end);
      i = end;
      continue;
    }

    if (syntax.block && content.startsWith(syntax.block[0], i)) {
      const close = content.indexOf(syntax.block[1], i + syntax.block[0].length);
      const end = close === -1 ? length : close + syntax.block[1].length;
      const body = content.slice(i + syntax.block[0].length, close === -1 ? length : close);
      const firstLine = lineOf(i);
      for (const [offset, text] of body.split("\n").entries()) {
        comments.push({ line: firstLine + offset, text });
      }
      blank(i, end);
      i = end;
      continue;
    }

    if (syntax.tripleQuotes && (content.startsWith('"""', i) || content.startsWith("'''", i))) {
      const delimiter = content.slice(i, i + 3);
      const close = content.indexOf(delimiter, i + 3);
      const end = close === -1 ? length : close + 3;
      strings.push({ value: content.slice(i + 3, close === -1 ? length : close), start: i, end });
      blank(i + 3, close === -1 ? length : close);
      lastSignificant = '"';
      i = end;
      continue;
    }

    if (syntax.quotes.includes(ch) || (syntax.backtick && ch === "`")) {
      if (
        ch === "'" &&
        syntax.charLiteralQuote &&
        !/^'(?:\\.|[^'\\])'/.test(content.slice(i, i + 5))
      ) {
        i += 1;
        continue;
      }
      const multiline = ch === "`";
      let j = i + 1;
      while (j < length) {
        const current = content[j];
        if (current === "\\") {
          j += 2;
          continue;
        }
        if (current === ch) break;
        if (current === "\n" && !multiline) break;
        j += 1;
      }
      const closed = content[j] === ch;
      const end = closed ? j + 1 : j;
      strings.push({ value: content.slice(i + 1, closed ? j : end), start: i, end });
      blank(i + 1, closed ? j : end);
      lastSignificant = ch;
      i = end;
      continue;
    }

    if (
      syntax.regexLiterals &&
      ch === "/" &&
      (lastSignificant === "" ||
        REGEX_PRECEDERS.has(lastSignificant) ||
        REGEX_KEYWORDS.test(content.slice(Math.max(0, i - 12), i).trimEnd()))
    ) {
      let j = i + 1;
      let inClass = false;
      while (j < length && content[j] !== "\n") {
        const current = content[j];
        if (current === "\\") {
          j += 2;
          continue;
        }
        if (current === "[") inClass = true;
        else if (current === "]") inClass = false;
        else if (current === "/" && !inClass) break;
        j += 1;
      }
      if (content[j] === "/") {
        blank(i + 1, j);
        lastSignificant = "/";
        i = j + 1;
        continue;
      }
    }

    if (ch !== " " && ch !== "\t" && ch !== "\n" && ch !== "\r") lastSignificant = ch;
    i += 1;
  }

  return { masked: chars.join(""), strings, comments };
}
