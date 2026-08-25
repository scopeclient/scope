//! Discord-flavoured markdown → [`Block`]s.
//!
//! A hand-written parser in two layers: a line-based block scanner (code
//! fences, headings, subtext, quotes, lists, paragraphs) on top of a recursive
//! inline parser (styles, spoilers, code spans, links, mentions, emoji,
//! timestamps). It follows what Discord renders rather than CommonMark:
//! unclosed markers are literal text, `_` only italicises at word boundaries,
//! `*` needs a non-space right after it, and the first matching closer wins.

use chrono::{DateTime, Local};

use crate::model::{Block, Emoji, Inline, Mention, TextStyle, TimestampStyle};

/// Resolves mention ids to display names; backends implement this over their cache.
pub trait MentionResolver {
  fn user(&self, id: u64) -> Option<String>;
  fn channel(&self, id: u64) -> Option<String>;
  /// Role name and `0xRRGGBB` colour.
  fn role(&self, id: u64) -> Option<(String, Option<u32>)>;
  /// Image for a custom emoji; `None` means "use the Discord CDN".
  fn custom_emoji_url(&self, _id: u64, _animated: bool) -> Option<String> {
    None
  }
}

/// Resolver that knows nothing; mentions fall back to their ids.
pub struct NoResolver;

impl MentionResolver for NoResolver {
  fn user(&self, _: u64) -> Option<String> {
    None
  }

  fn channel(&self, _: u64) -> Option<String> {
    None
  }

  fn role(&self, _: u64) -> Option<(String, Option<u32>)> {
    None
  }
}

/// Parses a whole message body into blocks. Empty or whitespace-only input yields no blocks.
pub fn parse(source: &str, resolver: &dyn MentionResolver) -> Vec<Block> {
  if source.trim().is_empty() {
    return Vec::new();
  }
  let source = source.replace("\r\n", "\n").replace('\r', "\n");
  BlockParser { resolver }.parse_lines(source.split('\n').collect(), false)
}

/// Parses a single run of inline markdown; `\n` becomes [`Inline::LineBreak`].
pub fn parse_inline(source: &str, resolver: &dyn MentionResolver) -> Vec<Inline> {
  InlineParser {
    src: source,
    bytes: source.as_bytes(),
    resolver,
  }
  .parse()
}

// ---- blocks -------------------------------------------------------------------

struct BlockParser<'a> {
  resolver: &'a dyn MentionResolver,
}

/// One `- item` / `1. item` line plus any continuation lines.
struct ListLine {
  indent: usize,
  /// `Some(n)` for `n.` markers, `None` for `-` / `*`.
  number: Option<u32>,
  text: String,
}

impl BlockParser<'_> {
  fn inline(&self, text: &str) -> Vec<Inline> {
    parse_inline(text, self.resolver)
  }

  fn flush(&self, paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>) {
    if paragraph.is_empty() {
      return;
    }
    let text = paragraph.join("\n");
    paragraph.clear();
    blocks.push(Block::Paragraph(self.inline(&text)));
  }

  /// `in_quote` disables the quote rule so `> > x` shows a literal `> x`, as Discord does.
  fn parse_lines<'s>(&self, mut lines: Vec<&'s str>, in_quote: bool) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&'s str> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
      let line = lines[i];

      if line.trim().is_empty() {
        self.flush(&mut paragraph, &mut blocks);
        i += 1;
        continue;
      }

      if let Some(rest) = line.trim_start_matches(' ').strip_prefix("```")
        && let Some((block, next, trailing)) = self.fence(rest, &lines, i)
      {
        self.flush(&mut paragraph, &mut blocks);
        blocks.push(block);
        i = next;
        // Text after a closing fence on the same line is re-scanned as its own line.
        if !trailing.trim().is_empty() {
          lines[i - 1] = trailing.trim_start();
          i -= 1;
        }
        continue;
      }

      if let Some((level, content)) = heading(line) {
        self.flush(&mut paragraph, &mut blocks);
        blocks.push(Block::Heading {
          level,
          content: self.inline(content),
        });
        i += 1;
        continue;
      }

      if let Some(content) = subtext(line) {
        self.flush(&mut paragraph, &mut blocks);
        blocks.push(Block::Subtext(self.inline(content)));
        i += 1;
        continue;
      }

      if !in_quote && let Some((rest_of_message, content)) = quote(line) {
        self.flush(&mut paragraph, &mut blocks);
        let mut inner = vec![content];
        let mut j = i + 1;
        if rest_of_message {
          inner.extend_from_slice(&lines[j..]);
          j = lines.len();
        } else {
          while j < lines.len()
            && let Some((false, content)) = quote(lines[j])
          {
            inner.push(content);
            j += 1;
          }
        }
        blocks.push(Block::Quote(self.parse_lines(inner, true)));
        i = j;
        continue;
      }

      if list_item(line).is_some() {
        self.flush(&mut paragraph, &mut blocks);
        let mut items: Vec<ListLine> = Vec::new();
        let mut j = i;
        while j < lines.len() {
          let l = lines[j];
          if let Some((indent, number, text)) = list_item(l) {
            items.push(ListLine {
              indent,
              number,
              text: text.to_string(),
            });
          } else if !l.trim().is_empty() && !is_block_start(l) {
            // Lazy continuation: plain text under an item belongs to it.
            let last = items.last_mut().expect("list starts with an item");
            last.text.push('\n');
            last.text.push_str(l.trim_start());
          } else {
            break;
          }
          j += 1;
        }
        blocks.extend(self.build_lists(&items));
        i = j;
        continue;
      }

      paragraph.push(line);
      i += 1;
    }

    self.flush(&mut paragraph, &mut blocks);
    blocks
  }

  /// `rest` is the text after the opening backticks of line `i`. Returns the
  /// block, the index of the line after the fence, and any text that followed
  /// the closing fence on its line. `None` when the fence is never closed.
  fn fence<'s>(&self, rest: &'s str, lines: &[&'s str], i: usize) -> Option<(Block, usize, &'s str)> {
    if let Some(end) = rest.find("```") {
      // ```code``` on one line: no language.
      let code = &rest[..end];
      if code.is_empty() {
        return None;
      }
      return Some((
        Block::CodeBlock {
          language: None,
          code: code.to_string(),
        },
        i + 1,
        &rest[end + 3..],
      ));
    }

    let close = (i + 1..lines.len()).find(|&j| lines[j].contains("```"))?;
    let mut code_lines: Vec<&str> = Vec::new();
    let mut language = None;
    if !rest.is_empty() {
      if rest.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'-' | b'.' | b'#')) {
        language = Some(rest.to_string());
      } else {
        code_lines.push(rest);
      }
    }
    code_lines.extend_from_slice(&lines[i + 1..close]);
    let closing = lines[close];
    let end = closing.find("```").expect("line contains a fence");
    if !closing[..end].is_empty() {
      code_lines.push(&closing[..end]);
    }

    // Leading and trailing blank lines are not part of the code.
    let first = code_lines.iter().position(|l| !l.trim().is_empty());
    let last = code_lines.iter().rposition(|l| !l.trim().is_empty());
    let mut code = match (first, last) {
      (Some(a), Some(b)) => code_lines[a..=b].join("\n"),
      _ => String::new(),
    };
    if code.is_empty() {
      // ```rust\n``` is a code block containing "rust".
      code = language.take()?;
    }
    Some((Block::CodeBlock { language, code }, close + 1, &closing[end + 3..]))
  }

  /// Groups list lines into `List` blocks. Items indented deeper than the first
  /// become a nested list inside the preceding item; a change between ordered
  /// and unordered markers starts a new list.
  fn build_lists(&self, items: &[ListLine]) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let Some(base) = items.first().map(|l| l.indent) else {
      return out;
    };
    let mut k = 0;
    while k < items.len() {
      let item = &items[k];
      let mut content = vec![Block::Paragraph(self.inline(&item.text))];
      let nested_end = (k + 1..items.len()).find(|&n| items[n].indent <= base).unwrap_or(items.len());
      if nested_end > k + 1 {
        content.extend(self.build_lists(&items[k + 1..nested_end]));
      }
      let ordered = item.number.is_some();
      match out.last_mut() {
        Some(Block::List { ordered: o, items, .. }) if *o == ordered => items.push(content),
        _ => out.push(Block::List {
          ordered,
          start: item.number.unwrap_or(1),
          items: vec![content],
        }),
      }
      k = nested_end;
    }
    out
  }
}

/// `# `, `## `, `### ` followed by text that does not itself start with `#`.
fn heading(line: &str) -> Option<(u8, &str)> {
  let s = line.trim_start_matches(' ');
  let hashes = s.bytes().take_while(|&b| b == b'#').count();
  if !(1..=3).contains(&hashes) {
    return None;
  }
  let rest = &s[hashes..];
  if !rest.starts_with([' ', '\t']) {
    return None;
  }
  let content = rest.trim_start();
  if content.is_empty() || content.starts_with('#') {
    return None;
  }
  Some((hashes as u8, content))
}

/// `-# small text`.
fn subtext(line: &str) -> Option<&str> {
  let rest = line.trim_start_matches(' ').strip_prefix("-#")?;
  if !rest.starts_with([' ', '\t']) {
    return None;
  }
  let content = rest.trim_start();
  (!content.is_empty()).then_some(content)
}

/// `> text` or `>>> text`; returns `(quotes_rest_of_message, content)`.
fn quote(line: &str) -> Option<(bool, &str)> {
  let s = line.trim_start_matches(' ');
  if let Some(rest) = s.strip_prefix(">>>") {
    return rest.starts_with(' ').then(|| (true, rest.trim_start_matches(' ')));
  }
  let rest = s.strip_prefix('>')?;
  rest.starts_with(' ').then(|| (false, rest.trim_start_matches(' ')))
}

/// `- item`, `* item`, `1. item`; returns `(indent, number, text)`.
fn list_item(line: &str) -> Option<(usize, Option<u32>, &str)> {
  let indent = line.bytes().take_while(|b| matches!(b, b' ' | b'\t')).map(|b| if b == b'\t' { 2 } else { 1 }).sum();
  let s = line.trim_start_matches([' ', '\t']);
  let (number, rest) = if let Some(rest) = s.strip_prefix(['-', '*']) {
    (None, rest)
  } else {
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
      return None;
    }
    (Some(s[..digits].parse().ok()?), s[digits..].strip_prefix('.')?)
  };
  if !rest.starts_with(' ') {
    return None;
  }
  let text = rest.trim_start_matches(' ');
  (!text.is_empty()).then_some((indent, number, text))
}

/// A line that ends a list's lazy continuation.
fn is_block_start(line: &str) -> bool {
  line.trim_start_matches(' ').starts_with("```") || heading(line).is_some() || subtext(line).is_some() || quote(line).is_some()
}

// ---- inlines ------------------------------------------------------------------

struct InlineParser<'a> {
  src: &'a str,
  bytes: &'a [u8],
  resolver: &'a dyn MentionResolver,
}

enum Delim {
  Style(TextStyle),
  Spoiler,
}

fn is_word(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn parse_id(s: &str) -> Option<u64> {
  (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

impl<'a> InlineParser<'a> {
  fn parse(&self) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;

    // Every rule starts on an ASCII byte and advances past whole characters,
    // so `i` is always on a char boundary.
    while i < self.bytes.len() {
      match self.rule(i) {
        Some((Inline::Text(t), next)) => {
          text.push_str(&t);
          i = next;
        }
        Some((inline, next)) => {
          if !text.is_empty() {
            out.push(Inline::Text(std::mem::take(&mut text)));
          }
          out.push(inline);
          i = next;
        }
        None => {
          let ch = self.src[i..].chars().next().expect("in bounds");
          text.push(ch);
          i += ch.len_utf8();
        }
      }
    }

    if !text.is_empty() {
      out.push(Inline::Text(text));
    }
    out
  }

  fn sub(&self, src: &'a str) -> Vec<Inline> {
    InlineParser {
      src,
      bytes: src.as_bytes(),
      resolver: self.resolver,
    }
    .parse()
  }

  fn rule(&self, i: usize) -> Option<(Inline, usize)> {
    match self.bytes[i] {
      b'\\' => self.escape(i),
      b'\n' => Some((Inline::LineBreak, i + 1)),
      b'`' => self.code(i),
      b'<' => self.angle(i),
      b'@' => self.at_mention(i),
      b'[' => self.masked_link(i),
      b'h' => self.bare_url(i),
      b'*' | b'_' | b'~' | b'|' => self.styled(i),
      _ => None,
    }
  }

  /// `\*` → `*`; a backslash before a letter, digit or space stays literal.
  fn escape(&self, i: usize) -> Option<(Inline, usize)> {
    let ch = self.src[i + 1..].chars().next()?;
    if ch.is_alphanumeric() || ch.is_whitespace() {
      return None;
    }
    Some((Inline::Text(ch.to_string()), i + 1 + ch.len_utf8()))
  }

  /// A run of N backticks closed by another run of exactly N on the same line.
  fn code(&self, i: usize) -> Option<(Inline, usize)> {
    let run = |at: usize| self.bytes[at..].iter().take_while(|&&b| b == b'`').count();
    let ticks = run(i);
    let from = i + ticks;
    let mut j = from;
    while j < self.bytes.len() {
      match self.bytes[j] {
        b'\n' => return None,
        b'`' => {
          let n = run(j);
          if n == ticks && j > from {
            return Some((Inline::Code(self.src[from..j].to_string()), j + n));
          }
          j += n;
        }
        _ => j += 1,
      }
    }
    None
  }

  /// `<...>` forms: mentions, commands, custom emoji, timestamps, suppressed-embed links.
  fn angle(&self, i: usize) -> Option<(Inline, usize)> {
    let end = i + 1 + self.bytes[i + 1..].iter().position(|&b| b == b'>')?;
    let inner = &self.src[i + 1..end];
    let next = end + 1;

    let inline = if let Some(id) = inner.strip_prefix("@&") {
      let id = parse_id(id)?;
      let (name, color) = self.resolver.role(id).unwrap_or_else(|| ("role".to_string(), None));
      Inline::Mention(Mention::Role { id, name, color })
    } else if let Some(id) = inner.strip_prefix("@!").or_else(|| inner.strip_prefix('@')) {
      let id = parse_id(id)?;
      let name = self.resolver.user(id).unwrap_or_else(|| id.to_string());
      Inline::Mention(Mention::User { id, name })
    } else if let Some(id) = inner.strip_prefix('#') {
      let id = parse_id(id)?;
      let name = self.resolver.channel(id).unwrap_or_else(|| id.to_string());
      Inline::Mention(Mention::Channel { id, name })
    } else if let Some(rest) = inner.strip_prefix('/') {
      let (name, id) = rest.rsplit_once(':')?;
      parse_id(id)?;
      let valid = !name.is_empty() && !name.starts_with(' ') && name.bytes().all(|b| is_word(b) || matches!(b, b'-' | b' '));
      if !valid {
        return None;
      }
      Inline::Mention(Mention::Command { name: name.to_string() })
    } else if let Some(rest) = inner.strip_prefix("a:").or_else(|| inner.strip_prefix(':')) {
      let animated = inner.starts_with('a');
      let (name, id) = rest.split_once(':')?;
      let id = parse_id(id)?;
      if name.is_empty() || !name.bytes().all(|b| is_word(b) || b == b'~') {
        return None;
      }
      Inline::Emoji(Emoji::Custom {
        id,
        name: name.to_string(),
        animated,
        url: self.resolver.custom_emoji_url(id, animated),
      })
    } else if let Some(rest) = inner.strip_prefix("t:") {
      let (unix, style) = match rest.split_once(':') {
        Some((unix, style)) => (unix, Some(style)),
        None => (rest, None),
      };
      let digits = unix.strip_prefix('-').unwrap_or(unix);
      if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
      }
      let unix: i64 = unix.parse().ok()?;
      let style = match style {
        None => TimestampStyle::Default,
        Some("t") => TimestampStyle::ShortTime,
        Some("T") => TimestampStyle::LongTime,
        Some("d") => TimestampStyle::ShortDate,
        Some("D") => TimestampStyle::LongDate,
        Some("f") => TimestampStyle::ShortDateTime,
        Some("F") => TimestampStyle::LongDateTime,
        Some("R") => TimestampStyle::Relative,
        Some(_) => return None,
      };
      Inline::Timestamp { unix, style }
    } else if (inner.starts_with("http://") || inner.starts_with("https://")) && !inner.bytes().any(|b| b.is_ascii_whitespace() || b == b'<') {
      Inline::Link {
        url: inner.to_string(),
        label: None,
      }
    } else {
      return None;
    };

    Some((inline, next))
  }

  fn at_mention(&self, i: usize) -> Option<(Inline, usize)> {
    let rest = &self.src[i..];
    let (mention, len) = if rest.starts_with("@everyone") {
      (Mention::Everyone, "@everyone".len())
    } else if rest.starts_with("@here") {
      (Mention::Here, "@here".len())
    } else {
      return None;
    };
    if self.bytes.get(i + len).is_some_and(|&b| is_word(b)) {
      return None;
    }
    Some((Inline::Mention(mention), i + len))
  }

  /// `[label](url)` — only http, https and discord URLs become links.
  fn masked_link(&self, i: usize) -> Option<(Inline, usize)> {
    let label_end = i + 1 + self.bytes[i + 1..].iter().position(|&b| b == b']' || b == b'[' || b == b'\n')?;
    if self.bytes[label_end] != b']' || self.bytes.get(label_end + 1) != Some(&b'(') {
      return None;
    }
    let url_start = label_end + 2;
    let url_end = url_start + self.bytes[url_start..].iter().position(|&b| b == b')' || b.is_ascii_whitespace())?;
    if self.bytes[url_end] != b')' {
      return None;
    }
    let label = &self.src[i + 1..label_end];
    let url = self.src[url_start..url_end].trim_start_matches('<').trim_end_matches('>');
    let scheme_ok = ["http://", "https://", "discord://"].iter().any(|s| url.starts_with(s));
    if label.is_empty() || !scheme_ok {
      return None;
    }
    Some((
      Inline::Link {
        url: url.to_string(),
        label: Some(self.sub(label)),
      },
      url_end + 1,
    ))
  }

  /// `http(s)://…` up to whitespace, `<` or `>`, minus trailing punctuation.
  fn bare_url(&self, i: usize) -> Option<(Inline, usize)> {
    let rest = &self.src[i..];
    let scheme = if rest.starts_with("https://") {
      8
    } else if rest.starts_with("http://") {
      7
    } else {
      return None;
    };
    let start = i + scheme;
    let mut end =
      start + self.bytes[start..].iter().position(|&b| b.is_ascii_whitespace() || b == b'<' || b == b'>').unwrap_or(self.bytes.len() - start);
    while end > start && matches!(self.bytes[end - 1], b'.' | b',' | b':' | b';' | b'"' | b'\'' | b')' | b']') {
      end -= 1;
    }
    if end == start {
      return None;
    }
    Some((
      Inline::Link {
        url: self.src[i..end].to_string(),
        label: None,
      },
      end,
    ))
  }

  /// `**`, `*`, `_`, `__`, `~~`, `||` spans.
  fn styled(&self, i: usize) -> Option<(Inline, usize)> {
    let rest = &self.bytes[i..];
    let (delim, kind): (&[u8], Delim) = if rest.starts_with(b"**") {
      (b"**", Delim::Style(TextStyle::Bold))
    } else if rest.starts_with(b"__") {
      (b"__", Delim::Style(TextStyle::Underline))
    } else if rest.starts_with(b"~~") {
      (b"~~", Delim::Style(TextStyle::Strikethrough))
    } else if rest.starts_with(b"||") {
      (b"||", Delim::Spoiler)
    } else if rest[0] == b'*' || rest[0] == b'_' {
      (&rest[..1], Delim::Style(TextStyle::Italic))
    } else {
      return None;
    };

    let from = i + delim.len();
    let close = match delim {
      b"**" => self.find_close(from, delim, None, |after| after != Some(b'*'), |_| true),
      b"__" => self.find_close(from, delim, None, |after| after != Some(b'_'), |_| true),
      b"~~" => self.find_close(from, delim, None, |after| after != Some(b'~'), |_| true),
      b"||" => self.find_close(from, delim, None, |_| true, |_| true),
      b"*" => {
        // `* text*` is not italic: the opener needs a non-space after it and
        // the closer a non-space before it.
        if self.bytes.get(from).is_none_or(|b| b.is_ascii_whitespace()) {
          return None;
        }
        self.find_close(from, delim, Some(b"**"), |_| true, |before| !before.is_ascii_whitespace())
      }
      _ => {
        // `_` only works at word boundaries, so snake_case stays literal.
        if i > 0 && is_word(self.bytes[i - 1]) {
          return None;
        }
        self.find_close(from, delim, Some(b"__"), |after| !after.is_some_and(is_word), |_| true)
      }
    }?;

    let content = self.sub(&self.src[from..close]);
    let inline = match kind {
      Delim::Style(style) => Inline::Styled { style, content },
      Delim::Spoiler => Inline::Spoiler(content),
    };
    Some((inline, close + delim.len()))
  }

  /// Finds the first `delim` at or after `from` that satisfies both guards,
  /// skipping escaped characters and `skip` runs (`**` inside `*…*`).
  fn find_close(
    &self,
    from: usize,
    delim: &[u8],
    skip: Option<&[u8]>,
    after_ok: impl Fn(Option<u8>) -> bool,
    before_ok: impl Fn(u8) -> bool,
  ) -> Option<usize> {
    let bytes = self.bytes;
    let mut j = from;
    while j < bytes.len() {
      if bytes[j] == b'\\' {
        j += 2;
        continue;
      }
      if let Some(skip) = skip
        && bytes[j..].starts_with(skip)
      {
        j += skip.len();
        continue;
      }
      if bytes[j..].starts_with(delim) && j > from && before_ok(bytes[j - 1]) && after_ok(bytes.get(j + delim.len()).copied()) {
        return Some(j);
      }
      j += 1;
    }
    None
  }
}

// ---- plain text ------------------------------------------------------------------

/// Flatten blocks back to plain text (reply snippets, notifications, search).
pub fn to_plain_text(blocks: &[Block]) -> String {
  fn inline(out: &mut String, inlines: &[Inline]) {
    for i in inlines {
      match i {
        Inline::Text(t) | Inline::Code(t) => out.push_str(t),
        Inline::Styled { content, .. } | Inline::Spoiler(content) => inline(out, content),
        Inline::Link { url, label } => match label {
          Some(label) => inline(out, label),
          None => out.push_str(url),
        },
        Inline::Mention(m) => out.push_str(&match m {
          Mention::User { name, .. } => format!("@{name}"),
          Mention::Channel { name, .. } => format!("#{name}"),
          Mention::Role { name, .. } => format!("@{name}"),
          Mention::Command { name } => format!("/{name}"),
          Mention::Everyone => "@everyone".into(),
          Mention::Here => "@here".into(),
        }),
        Inline::Emoji(e) => out.push_str(&e.label()),
        Inline::Timestamp { unix, style } => out.push_str(&format_timestamp(*unix, *style)),
        Inline::LineBreak => out.push('\n'),
      }
    }
  }

  let mut out = String::new();
  for (n, block) in blocks.iter().enumerate() {
    if n > 0 {
      out.push('\n');
    }
    match block {
      Block::Paragraph(c) | Block::Heading { content: c, .. } | Block::Subtext(c) => inline(&mut out, c),
      Block::Quote(inner) => out.push_str(&to_plain_text(inner)),
      Block::CodeBlock { code, .. } => out.push_str(code),
      Block::List { items, .. } => {
        for (k, item) in items.iter().enumerate() {
          if k > 0 {
            out.push('\n');
          }
          out.push_str(&to_plain_text(item));
        }
      }
    }
  }
  out
}

/// Absolute local time in roughly Discord's format for each style; relative
/// timestamps fall back to the short date-time.
pub fn format_timestamp(unix: i64, style: TimestampStyle) -> String {
  let Some(time) = DateTime::from_timestamp(unix, 0) else {
    return "[time]".to_string();
  };
  let local = time.with_timezone(&Local);
  let pattern = match style {
    TimestampStyle::ShortTime => "%H:%M",
    TimestampStyle::LongTime => "%H:%M:%S",
    TimestampStyle::ShortDate => "%d/%m/%Y",
    TimestampStyle::LongDate => "%-d %B %Y",
    TimestampStyle::LongDateTime => "%A, %-d %B %Y %H:%M",
    TimestampStyle::ShortDateTime | TimestampStyle::Default | TimestampStyle::Relative => "%-d %B %Y %H:%M",
  };
  local.format(pattern).to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Stub;

  impl MentionResolver for Stub {
    fn user(&self, id: u64) -> Option<String> {
      (id == 1).then(|| "alice".to_string())
    }

    fn channel(&self, id: u64) -> Option<String> {
      (id == 3).then(|| "general".to_string())
    }

    fn role(&self, id: u64) -> Option<(String, Option<u32>)> {
      (id == 4).then(|| ("admin".to_string(), Some(0xff0000)))
    }
  }

  fn t(s: &str) -> Inline {
    Inline::Text(s.to_string())
  }

  fn styled(style: TextStyle, content: Vec<Inline>) -> Inline {
    Inline::Styled { style, content }
  }

  fn p(content: Vec<Inline>) -> Block {
    Block::Paragraph(content)
  }

  fn parse_one(src: &str) -> Vec<Inline> {
    let blocks = parse(src, &NoResolver);
    assert_eq!(blocks.len(), 1, "expected one block for {src:?}, got {blocks:?}");
    match blocks.into_iter().next().unwrap() {
      Block::Paragraph(c) => c,
      other => panic!("expected paragraph, got {other:?}"),
    }
  }

  #[test]
  fn empty_and_whitespace_yield_nothing() {
    assert!(parse("", &NoResolver).is_empty());
    assert!(parse("  \n\n \t", &NoResolver).is_empty());
  }

  #[test]
  fn plain_paragraph_and_blank_line_split() {
    assert_eq!(parse("hello world", &NoResolver), vec![p(vec![t("hello world")])]);
    assert_eq!(
      parse("one\n\ntwo\n\n\nthree\n", &NoResolver),
      vec![p(vec![t("one")]), p(vec![t("two")]), p(vec![t("three")])]
    );
  }

  #[test]
  fn single_newline_is_a_line_break() {
    assert_eq!(parse_one("a\nb"), vec![t("a"), Inline::LineBreak, t("b")]);
    assert_eq!(parse_one("a\r\nb"), vec![t("a"), Inline::LineBreak, t("b")]);
  }

  #[test]
  fn bold_italic_underline_strike() {
    assert_eq!(parse_one("**bold**"), vec![styled(TextStyle::Bold, vec![t("bold")])]);
    assert_eq!(
      parse_one("*it* and _it_"),
      vec![
        styled(TextStyle::Italic, vec![t("it")]),
        t(" and "),
        styled(TextStyle::Italic, vec![t("it")])
      ]
    );
    assert_eq!(parse_one("__u__"), vec![styled(TextStyle::Underline, vec![t("u")])]);
    assert_eq!(parse_one("~~gone~~"), vec![styled(TextStyle::Strikethrough, vec![t("gone")])]);
  }

  #[test]
  fn bold_italic_combo_and_nesting() {
    assert_eq!(
      parse_one("***both***"),
      vec![styled(TextStyle::Bold, vec![styled(TextStyle::Italic, vec![t("both")])])]
    );
    assert_eq!(
      parse_one("**bold *italic* bold**"),
      vec![styled(
        TextStyle::Bold,
        vec![t("bold "), styled(TextStyle::Italic, vec![t("italic")]), t(" bold")]
      )]
    );
    assert_eq!(
      parse_one("*italic **bold** italic*"),
      vec![styled(
        TextStyle::Italic,
        vec![t("italic "), styled(TextStyle::Bold, vec![t("bold")]), t(" italic")]
      )]
    );
    assert_eq!(
      parse_one("__~~**x**~~__"),
      vec![styled(
        TextStyle::Underline,
        vec![styled(TextStyle::Strikethrough, vec![styled(TextStyle::Bold, vec![t("x")])])]
      )]
    );
  }

  #[test]
  fn styles_span_line_breaks() {
    assert_eq!(
      parse_one("**a\nb**"),
      vec![styled(TextStyle::Bold, vec![t("a"), Inline::LineBreak, t("b")])]
    );
  }

  #[test]
  fn spoiler() {
    assert_eq!(
      parse_one("a ||secret **bold**|| b"),
      vec![
        t("a "),
        Inline::Spoiler(vec![t("secret "), styled(TextStyle::Bold, vec![t("bold")])]),
        t(" b")
      ]
    );
  }

  #[test]
  fn inline_code_has_no_formatting() {
    assert_eq!(parse_one("`**not bold** <@1>`"), vec![Inline::Code("**not bold** <@1>".to_string())]);
    assert_eq!(parse_one("``has ` tick``"), vec![Inline::Code("has ` tick".to_string())]);
    assert_eq!(parse_one("x ```three``` y"), vec![t("x "), Inline::Code("three".to_string()), t(" y")]);
    // Unclosed and cross-line backticks stay literal.
    assert_eq!(parse_one("`open"), vec![t("`open")]);
    assert_eq!(parse_one("`a\nb`"), vec![t("`a"), Inline::LineBreak, t("b`")]);
  }

  #[test]
  fn escapes_and_text_merging() {
    assert_eq!(parse_one("\\*not\\* \\_it\\_ \\\\ \\a"), vec![t("*not* _it_ \\ \\a")]);
    assert_eq!(parse_one("\\<@1> \\`x\\`"), vec![t("<@1> `x`")]);
  }

  #[test]
  fn unclosed_markers_are_literal() {
    assert_eq!(parse_one("**oops"), vec![t("**oops")]);
    assert_eq!(parse_one("~~nope and ||half"), vec![t("~~nope and ||half")]);
    assert_eq!(parse_one("****"), vec![t("****")]);
    assert_eq!(parse_one("a * b * c"), vec![t("a * b * c")]);
    assert_eq!(parse_one("2*3*4"), vec![t("2"), styled(TextStyle::Italic, vec![t("3")]), t("4")]);
  }

  #[test]
  fn underscore_needs_word_boundaries() {
    assert_eq!(parse_one("snake_case_name"), vec![t("snake_case_name")]);
    assert_eq!(parse_one("_a_b"), vec![t("_a_b")]);
    assert_eq!(parse_one("(_yes_)"), vec![t("("), styled(TextStyle::Italic, vec![t("yes")]), t(")")]);
  }

  #[test]
  fn bare_urls_stop_at_whitespace_and_trailing_punctuation() {
    let link = |u: &str| Inline::Link {
      url: u.to_string(),
      label: None,
    };
    assert_eq!(
      parse_one("see https://example.com/a?b=1, and (http://x.io)."),
      vec![t("see "), link("https://example.com/a?b=1"), t(", and ("), link("http://x.io"), t(").")]
    );
    assert_eq!(parse_one("https://a.b>c"), vec![link("https://a.b"), t(">c")]);
    assert_eq!(parse_one("https:// nothing"), vec![t("https:// nothing")]);
    assert_eq!(parse_one("**https://x.com**"), vec![styled(TextStyle::Bold, vec![link("https://x.com")])]);
  }

  #[test]
  fn angle_bracket_url_is_a_link() {
    assert_eq!(
      parse_one("<https://example.com/p?q=1>"),
      vec![Inline::Link {
        url: "https://example.com/p?q=1".to_string(),
        label: None
      }]
    );
    assert_eq!(parse_one("<not a url>"), vec![t("<not a url>")]);
  }

  #[test]
  fn masked_links_need_an_allowed_scheme() {
    assert_eq!(
      parse_one("[**docs**](https://docs.rs) [x](<discord://-/users/1>)"),
      vec![
        Inline::Link {
          url: "https://docs.rs".to_string(),
          label: Some(vec![styled(TextStyle::Bold, vec![t("docs")])])
        },
        t(" "),
        Inline::Link {
          url: "discord://-/users/1".to_string(),
          label: Some(vec![t("x")])
        },
      ]
    );
    assert_eq!(parse_one("[x](javascript:alert(1))"), vec![t("[x](javascript:alert(1))")]);
    assert_eq!(
      parse_one("[just brackets] (https://a.b)"),
      vec![
        t("[just brackets] ("),
        Inline::Link {
          url: "https://a.b".to_string(),
          label: None
        },
        t(")")
      ]
    );
  }

  #[test]
  fn mentions_use_the_resolver() {
    let blocks = parse("<@1> <@!2> <#3> <#30> <@&4> <@&40>", &Stub);
    assert_eq!(
      blocks,
      vec![p(vec![
        Inline::Mention(Mention::User { id: 1, name: "alice".into() }),
        t(" "),
        Inline::Mention(Mention::User { id: 2, name: "2".into() }),
        t(" "),
        Inline::Mention(Mention::Channel {
          id: 3,
          name: "general".into()
        }),
        t(" "),
        Inline::Mention(Mention::Channel { id: 30, name: "30".into() }),
        t(" "),
        Inline::Mention(Mention::Role {
          id: 4,
          name: "admin".into(),
          color: Some(0xff0000)
        }),
        t(" "),
        Inline::Mention(Mention::Role {
          id: 40,
          name: "role".into(),
          color: None
        }),
      ])]
    );
    assert_eq!(parse_one("<@abc> <#> <@&x>"), vec![t("<@abc> <#> <@&x>")]);
  }

  #[test]
  fn command_mentions() {
    assert_eq!(
      parse_one("</ping:5> </config set:6>"),
      vec![
        Inline::Mention(Mention::Command { name: "ping".into() }),
        t(" "),
        Inline::Mention(Mention::Command { name: "config set".into() })
      ]
    );
    assert_eq!(parse_one("</ping>"), vec![t("</ping>")]);
  }

  #[test]
  fn everyone_and_here() {
    assert_eq!(
      parse_one("@everyone @here @everyonex @herein"),
      vec![
        Inline::Mention(Mention::Everyone),
        t(" "),
        Inline::Mention(Mention::Here),
        t(" @everyonex @herein")
      ]
    );
  }

  #[test]
  fn custom_emoji() {
    assert_eq!(
      parse_one("<:smile:10><a:party:11> <:bad name:1>"),
      vec![
        Inline::Emoji(Emoji::Custom {
          id: 10,
          name: "smile".into(),
          animated: false,
          url: None
        }),
        Inline::Emoji(Emoji::Custom {
          id: 11,
          name: "party".into(),
          animated: true,
          url: None
        }),
        t(" <:bad name:1>"),
      ]
    );
  }

  #[test]
  fn timestamps() {
    let ts = |style| Inline::Timestamp { unix: 1_700_000_000, style };
    assert_eq!(
      parse_one("<t:1700000000> <t:1700000000:R>"),
      vec![ts(TimestampStyle::Default), t(" "), ts(TimestampStyle::Relative)]
    );
    for (c, style) in [
      ("t", TimestampStyle::ShortTime),
      ("T", TimestampStyle::LongTime),
      ("d", TimestampStyle::ShortDate),
      ("D", TimestampStyle::LongDate),
      ("f", TimestampStyle::ShortDateTime),
      ("F", TimestampStyle::LongDateTime),
    ] {
      assert_eq!(parse_one(&format!("<t:1700000000:{c}>")), vec![ts(style)]);
    }
    assert_eq!(
      parse_one("<t:-1>"),
      vec![Inline::Timestamp {
        unix: -1,
        style: TimestampStyle::Default
      }]
    );
    assert_eq!(parse_one("<t:1700000000:x> <t:abc>"), vec![t("<t:1700000000:x> <t:abc>")]);
  }

  #[test]
  fn fenced_code_blocks() {
    let code = |language: Option<&str>, code: &str| Block::CodeBlock {
      language: language.map(str::to_string),
      code: code.to_string(),
    };
    assert_eq!(parse("```rust\nfn main() {}\n```", &NoResolver), vec![code(Some("rust"), "fn main() {}")]);
    assert_eq!(
      parse("```\n**raw** <@1>\n\n  indented\n```", &NoResolver),
      vec![code(None, "**raw** <@1>\n\n  indented")]
    );
    assert_eq!(parse("```one liner```", &NoResolver), vec![code(None, "one liner")]);
    assert_eq!(parse("```not a lang\nx```", &NoResolver), vec![code(None, "not a lang\nx")]);
    assert_eq!(parse("```rust\n```", &NoResolver), vec![code(None, "rust")]);
    // Text around the fence on the same lines.
    assert_eq!(
      parse("before\n```a``` after", &NoResolver),
      vec![p(vec![t("before")]), code(None, "a"), p(vec![t("after")])]
    );
    // Unclosed fences are literal text.
    assert_eq!(
      parse("```rust\nnever closed", &NoResolver),
      vec![p(vec![t("```rust"), Inline::LineBreak, t("never closed")])]
    );
  }

  #[test]
  fn headings_versus_hashtags() {
    assert_eq!(
      parse("# One\n## Two **bold**\n### Three", &NoResolver),
      vec![
        Block::Heading {
          level: 1,
          content: vec![t("One")]
        },
        Block::Heading {
          level: 2,
          content: vec![t("Two "), styled(TextStyle::Bold, vec![t("bold")])]
        },
        Block::Heading {
          level: 3,
          content: vec![t("Three")]
        },
      ]
    );
    assert_eq!(parse("#hashtag", &NoResolver), vec![p(vec![t("#hashtag")])]);
    assert_eq!(parse("#### four", &NoResolver), vec![p(vec![t("#### four")])]);
    assert_eq!(parse("# ", &NoResolver), vec![p(vec![t("# ")])]);
    assert_eq!(parse("text # not heading", &NoResolver), vec![p(vec![t("text # not heading")])]);
  }

  #[test]
  fn subtext_versus_list() {
    assert_eq!(parse("-# small", &NoResolver), vec![Block::Subtext(vec![t("small")])]);
    assert_eq!(
      parse("- item", &NoResolver),
      vec![Block::List {
        ordered: false,
        start: 1,
        items: vec![vec![p(vec![t("item")])]]
      }]
    );
    assert_eq!(parse("-#nope", &NoResolver), vec![p(vec![t("-#nope")])]);
  }

  #[test]
  fn quotes_merge_consecutive_lines() {
    assert_eq!(
      parse("> a\n> b\nafter\n> c", &NoResolver),
      vec![
        Block::Quote(vec![p(vec![t("a"), Inline::LineBreak, t("b")])]),
        p(vec![t("after")]),
        Block::Quote(vec![p(vec![t("c")])])
      ]
    );
    assert_eq!(
      parse("> # Title\n> - x", &NoResolver),
      vec![Block::Quote(vec![
        Block::Heading {
          level: 1,
          content: vec![t("Title")]
        },
        Block::List {
          ordered: false,
          start: 1,
          items: vec![vec![p(vec![t("x")])]]
        }
      ])]
    );
    assert_eq!(parse(">not quoted", &NoResolver), vec![p(vec![t(">not quoted")])]);
    assert_eq!(parse("> > inner", &NoResolver), vec![Block::Quote(vec![p(vec![t("> inner")])])]);
  }

  #[test]
  fn triple_quote_takes_the_rest() {
    assert_eq!(
      parse("intro\n>>> a\n\nb\n- c", &NoResolver),
      vec![
        p(vec![t("intro")]),
        Block::Quote(vec![
          p(vec![t("a")]),
          p(vec![t("b")]),
          Block::List {
            ordered: false,
            start: 1,
            items: vec![vec![p(vec![t("c")])]]
          }
        ])
      ]
    );
  }

  #[test]
  fn unordered_and_ordered_lists() {
    assert_eq!(
      parse("- a\n* b\n3. c\n4. d", &NoResolver),
      vec![
        Block::List {
          ordered: false,
          start: 1,
          items: vec![vec![p(vec![t("a")])], vec![p(vec![t("b")])]]
        },
        Block::List {
          ordered: true,
          start: 3,
          items: vec![vec![p(vec![t("c")])], vec![p(vec![t("d")])]]
        },
      ]
    );
    assert_eq!(
      parse("-no\n*no\n1.no", &NoResolver),
      vec![p(vec![t("-no"), Inline::LineBreak, t("*no"), Inline::LineBreak, t("1.no")])]
    );
    assert_eq!(
      parse("**bold** line", &NoResolver),
      vec![p(vec![styled(TextStyle::Bold, vec![t("bold")]), t(" line")])]
    );
  }

  #[test]
  fn nested_lists_and_continuation() {
    assert_eq!(
      parse("- a\n  - a1\n\t- a2\n- b\ncont\n\ntail", &NoResolver),
      vec![
        Block::List {
          ordered: false,
          start: 1,
          items: vec![
            vec![
              p(vec![t("a")]),
              Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![p(vec![t("a1")])], vec![p(vec![t("a2")])]]
              }
            ],
            vec![p(vec![t("b"), Inline::LineBreak, t("cont")])],
          ],
        },
        p(vec![t("tail")]),
      ]
    );
  }

  #[test]
  fn mixed_blocks() {
    assert_eq!(
      parse("intro line\nsecond\n# Head\n- one\n-# fine print\n\nend", &NoResolver),
      vec![
        p(vec![t("intro line"), Inline::LineBreak, t("second")]),
        Block::Heading {
          level: 1,
          content: vec![t("Head")]
        },
        Block::List {
          ordered: false,
          start: 1,
          items: vec![vec![p(vec![t("one")])]]
        },
        Block::Subtext(vec![t("fine print")]),
        p(vec![t("end")]),
      ]
    );
  }

  #[test]
  fn unicode_text_survives() {
    assert_eq!(
      parse_one("héllo **wörld** 🎉 \\— done"),
      vec![t("héllo "), styled(TextStyle::Bold, vec![t("wörld")]), t(" 🎉 — done")]
    );
  }

  #[test]
  fn plain_text_flattening() {
    let blocks = parse(
      "# Hi <@1>\n> in <#3> <:wave:9> [docs](https://d.rs) https://x.y\n- a\n- b\n```\ncode\n```",
      &Stub,
    );
    assert_eq!(to_plain_text(&blocks), "Hi @alice\nin #general :wave: docs https://x.y\na\nb\ncode");
    let time = to_plain_text(&parse("<t:1700000000:D>", &NoResolver));
    assert!(time.contains("2023"), "got {time:?}");
  }
}
