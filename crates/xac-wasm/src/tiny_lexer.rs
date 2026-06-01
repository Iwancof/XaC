use anyhow::{anyhow, Result};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TokenKind {
    Ident(String),
    Number(String),
    String(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    EqEq,
    Lt,
    Le,
    Gt,
    Ge,
    Eof,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) line: usize,
}

pub(crate) fn token_label(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Ident(_) => "identifier",
        TokenKind::Number(_) => "number",
        TokenKind::String(_) => "string",
        TokenKind::LParen => "(",
        TokenKind::RParen => ")",
        TokenKind::LBrace => "{",
        TokenKind::RBrace => "}",
        TokenKind::Comma => ",",
        TokenKind::Semicolon => ";",
        TokenKind::EqEq => "==",
        TokenKind::Lt => "<",
        TokenKind::Le => "<=",
        TokenKind::Gt => ">",
        TokenKind::Ge => ">=",
        TokenKind::Eof => "end of file",
    }
}

pub(crate) fn tokenize(source: &str) -> Result<Vec<Token>> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    let mut line = 1;

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' => {}
            '\n' => line += 1,
            '#' => skip_line_comment(&mut chars, &mut line),
            '/' => match chars.peek().copied() {
                Some('/') => {
                    chars.next();
                    skip_line_comment(&mut chars, &mut line);
                }
                Some('*') => {
                    chars.next();
                    skip_block_comment(&mut chars, &mut line)?;
                }
                _ => {
                    return Err(anyhow!("line {line}: unexpected /"));
                }
            },
            '(' => tokens.push(token(TokenKind::LParen, line)),
            ')' => tokens.push(token(TokenKind::RParen, line)),
            '{' => tokens.push(token(TokenKind::LBrace, line)),
            '}' => tokens.push(token(TokenKind::RBrace, line)),
            ',' => tokens.push(token(TokenKind::Comma, line)),
            ';' => tokens.push(token(TokenKind::Semicolon, line)),
            '<' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(token(TokenKind::Le, line));
                } else {
                    tokens.push(token(TokenKind::Lt, line));
                }
            }
            '>' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(token(TokenKind::Ge, line));
                } else {
                    tokens.push(token(TokenKind::Gt, line));
                }
            }
            '=' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(token(TokenKind::EqEq, line));
                } else {
                    return Err(anyhow!("line {line}: expected =="));
                }
            }
            '"' => {
                let string_line = line;
                tokens.push(token(
                    TokenKind::String(read_string(&mut chars, &mut line, string_line)?),
                    string_line,
                ));
            }
            ch if ch.is_ascii_alphabetic() || ch == '_' => {
                let mut ident = String::from(ch);
                while let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphanumeric() || next == '_' || next == '-' {
                        ident.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(token(TokenKind::Ident(normalize_ident(&ident)), line));
            }
            ch if ch.is_ascii_digit() => {
                let mut number = String::from(ch);
                let mut seen_dot = false;
                while let Some(next) = chars.peek().copied() {
                    if next.is_ascii_digit() {
                        number.push(next);
                        chars.next();
                    } else if next == '.' && !seen_dot {
                        seen_dot = true;
                        number.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(token(TokenKind::Number(number), line));
            }
            _ => return Err(anyhow!("line {line}: unexpected character {ch:?}")),
        }
    }

    tokens.push(token(TokenKind::Eof, line));
    Ok(tokens)
}

fn normalize_ident(value: &str) -> String {
    value.replace('-', "_").to_ascii_lowercase()
}

fn token(kind: TokenKind, line: usize) -> Token {
    Token { kind, line }
}

fn skip_line_comment<I>(chars: &mut std::iter::Peekable<I>, line: &mut usize)
where
    I: Iterator<Item = char>,
{
    for ch in chars.by_ref() {
        if ch == '\n' {
            *line += 1;
            break;
        }
    }
}

fn skip_block_comment<I>(chars: &mut std::iter::Peekable<I>, line: &mut usize) -> Result<()>
where
    I: Iterator<Item = char>,
{
    let mut previous = '\0';
    for ch in chars.by_ref() {
        if ch == '\n' {
            *line += 1;
        }
        if previous == '*' && ch == '/' {
            return Ok(());
        }
        previous = ch;
    }
    Err(anyhow!("line {line}: unterminated block comment"))
}

fn read_string<I>(
    chars: &mut std::iter::Peekable<I>,
    line: &mut usize,
    string_line: usize,
) -> Result<String>
where
    I: Iterator<Item = char>,
{
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Ok(out),
            '\\' => {
                let Some(escaped) = chars.next() else {
                    return Err(anyhow!("line {string_line}: unterminated string"));
                };
                match escaped {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push(' '),
                    't' => out.push(' '),
                    other => out.push(other),
                }
            }
            '\n' => {
                *line += 1;
                return Err(anyhow!("line {string_line}: string cannot span lines"));
            }
            other => out.push(other),
        }
    }
    Err(anyhow!("line {string_line}: unterminated string"))
}
