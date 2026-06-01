use anyhow::{anyhow, Result};
use xac_core::BehaviorKind;

use crate::script::compile_xac_script;
use crate::tiny_actions::{
    action_to_script, ident_arg, normalize_ident, number_arg, require_arg_count, Arg,
};
use crate::tiny_lexer::{token_label, tokenize, Token, TokenKind};

pub(crate) fn is_tiny_source(source: &str) -> bool {
    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("xac-lang: tiny")
            || lower.contains("xac_lang: tiny")
            || lower.contains("xac:lang=tiny")
        {
            return true;
        }
        if lower.starts_with("//")
            || lower.starts_with('#')
            || lower.starts_with("/*")
            || lower.starts_with('*')
        {
            continue;
        }
        return lower.starts_with("fn tick")
            || lower.starts_with("export fn tick")
            || lower.starts_with("pub fn tick")
            || lower.starts_with("void tick");
    }
    false
}

pub(crate) fn compile_tiny_source(kind: BehaviorKind, source: &str) -> Result<String> {
    let script_source = tiny_to_xac_script(source)?;
    compile_xac_script(kind, &script_source)
}

fn tiny_to_xac_script(source: &str) -> Result<String> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, pos: 0 };
    let script_lines = parser.parse_program()?;
    Ok(script_lines.join("\n"))
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn parse_program(&mut self) -> Result<Vec<String>> {
        self.skip_semicolons();
        match self.peek_ident() {
            Some("export") => {
                self.advance();
                self.expect_ident("fn")?;
                self.parse_tick_function_tail()
            }
            Some("pub") => {
                self.advance();
                self.expect_ident("fn")?;
                self.parse_tick_function_tail()
            }
            Some("fn") => {
                self.advance();
                self.parse_tick_function_tail()
            }
            Some("void") => {
                self.advance();
                self.parse_tick_function_tail()
            }
            _ => self.parse_statements_until_eof(),
        }
    }

    fn parse_tick_function_tail(&mut self) -> Result<Vec<String>> {
        self.expect_ident("tick")?;
        self.expect(TokenKind::LParen)?;
        self.expect(TokenKind::RParen)?;
        let lines = self.parse_block()?;
        self.skip_semicolons();
        self.expect(TokenKind::Eof)?;
        Ok(lines)
    }

    fn parse_statements_until_eof(&mut self) -> Result<Vec<String>> {
        let mut lines = Vec::new();
        while !self.at_eof() {
            lines.extend(self.parse_statement()?);
        }
        Ok(lines)
    }

    fn parse_block(&mut self) -> Result<Vec<String>> {
        self.expect(TokenKind::LBrace)?;
        let mut lines = Vec::new();
        while !self.consume(TokenKind::RBrace)? {
            if self.at_eof() {
                return Err(self.error_here("expected }"));
            }
            lines.extend(self.parse_statement()?);
        }
        Ok(lines)
    }

    fn parse_statement(&mut self) -> Result<Vec<String>> {
        self.skip_semicolons();
        if self.at_eof() || self.is_next(&TokenKind::RBrace) {
            return Ok(Vec::new());
        }
        if self.consume_ident("if") {
            self.expect(TokenKind::LParen)?;
            let condition = self.parse_condition()?;
            self.expect(TokenKind::RParen)?;
            if self.consume(TokenKind::LBrace)? {
                let mut lines = Vec::new();
                while !self.consume(TokenKind::RBrace)? {
                    if self.at_eof() {
                        return Err(self.error_here("expected }"));
                    }
                    let action = self.parse_action_statement()?;
                    lines.push(format!("if {condition} {action}"));
                }
                Ok(lines)
            } else {
                Ok(vec![format!(
                    "if {condition} {}",
                    self.parse_action_statement()?
                )])
            }
        } else {
            Ok(vec![self.parse_action_statement()?])
        }
    }

    fn parse_action_statement(&mut self) -> Result<String> {
        self.skip_semicolons();
        if self.consume_ident("return") {
            self.consume(TokenKind::Semicolon)?;
            return Ok("return".to_string());
        }
        let (name, args, line) = self.parse_call()?;
        self.consume(TokenKind::Semicolon)?;
        action_to_script(line, &name, &args)
    }

    fn parse_condition(&mut self) -> Result<String> {
        let (name, args, line) = self.parse_call()?;
        match name.as_str() {
            "output_blocked" | "can_produce" | "has_job" | "has_pending_job" => {
                require_arg_count(line, &name, &args, 0)?;
                Ok(name)
            }
            "ore_kind" => {
                require_arg_count(line, &name, &args, 0)?;
                self.expect(TokenKind::EqEq)?;
                Ok(format!("ore_kind == {}", self.expect_ident_any()?))
            }
            "output_available" => match args.as_slice() {
                [Arg::Ident(dir)] => Ok(format!("output_available {}", normalize_ident(dir))),
                [Arg::Ident(item), Arg::Ident(dir)] => Ok(format!(
                    "output_available {} {}",
                    normalize_ident(item),
                    normalize_ident(dir)
                )),
                _ => Err(anyhow!(
                    "line {line}: output_available expects (dir) or (item, dir)"
                )),
            },
            "can_attack" => {
                require_arg_count(line, &name, &args, 1)?;
                Ok(format!("can_attack {}", number_arg(line, &name, &args, 0)?))
            }
            "has_space" => {
                require_arg_count(line, &name, &args, 2)?;
                Ok(format!(
                    "has_space {} {}",
                    ident_arg(line, &name, &args, 0)?,
                    number_arg(line, &name, &args, 1)?
                ))
            }
            "input_count" | "output_count" | "inventory_count" | "stock_count"
            | "stock_capacity" | "cargo_count" => {
                require_arg_count(line, &name, &args, 1)?;
                let comparison = self.expect_comparison()?;
                let value = self.expect_number_any()?;
                Ok(format!(
                    "{name} {} {comparison} {value}",
                    ident_arg(line, &name, &args, 0)?
                ))
            }
            "inventory_free" | "docked_drone_count" | "pending_job_count" => {
                require_arg_count(line, &name, &args, 0)?;
                let comparison = self.expect_comparison()?;
                let value = self.expect_number_any()?;
                Ok(format!("{name} {comparison} {value}"))
            }
            "scan_enemies" => {
                require_arg_count(line, &name, &args, 0)?;
                let comparison = self.expect_comparison()?;
                let value = self.expect_number_any()?;
                Ok(format!("scan_enemies {comparison} {value}"))
            }
            "enemy_kind" => {
                require_arg_count(line, &name, &args, 1)?;
                self.expect(TokenKind::EqEq)?;
                Ok(format!(
                    "enemy_kind {} == {}",
                    number_arg(line, &name, &args, 0)?,
                    self.expect_ident_any()?
                ))
            }
            "enemy_hp" | "enemy_distance" => {
                require_arg_count(line, &name, &args, 1)?;
                let comparison = self.expect_comparison()?;
                let value = self.expect_number_any()?;
                Ok(format!(
                    "{name} {} {comparison} {value}",
                    number_arg(line, &name, &args, 0)?
                ))
            }
            "ammo_count" => {
                require_arg_count(line, &name, &args, 0)?;
                let comparison = self.expect_comparison()?;
                let value = self.expect_number_any()?;
                if comparison == ">" && value == "0" {
                    Ok("ammo_count > 0".to_string())
                } else {
                    Err(anyhow!(
                        "line {line}: ammo_count currently supports only > 0"
                    ))
                }
            }
            "battery_percent" => {
                require_arg_count(line, &name, &args, 0)?;
                self.expect_specific_comparison("<")?;
                Ok(format!("battery_percent < {}", self.expect_number_any()?))
            }
            "battery_ratio" => {
                require_arg_count(line, &name, &args, 0)?;
                self.expect_specific_comparison("<")?;
                Ok(format!("battery_ratio < {}", self.expect_number_any()?))
            }
            "logic_fuel_remaining" => {
                require_arg_count(line, &name, &args, 0)?;
                self.expect_specific_comparison("<")?;
                Ok(format!(
                    "logic_fuel_remaining < {}",
                    self.expect_number_any()?
                ))
            }
            "fuel_remaining" => {
                require_arg_count(line, &name, &args, 0)?;
                self.expect_specific_comparison(">")?;
                Ok(format!("fuel_remaining > {}", self.expect_number_any()?))
            }
            "net" | "net_get_i32" => {
                require_arg_count(line, &name, &args, 1)?;
                let comparison = self.expect_comparison()?;
                if comparison != ">" && comparison != "==" {
                    return Err(anyhow!("line {line}: net supports only > and =="));
                }
                Ok(format!(
                    "net {} {comparison} {}",
                    number_arg(line, &name, &args, 0)?,
                    self.expect_number_any()?
                ))
            }
            _ => Err(anyhow!("line {line}: unknown condition function {name}")),
        }
    }

    fn parse_call(&mut self) -> Result<(String, Vec<Arg>, usize)> {
        let line = self.current_line();
        let name = normalize_ident(&self.expect_ident_any()?);
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        if self.consume(TokenKind::RParen)? {
            return Ok((name, args, line));
        }
        loop {
            args.push(self.parse_arg()?);
            if self.consume(TokenKind::Comma)? {
                continue;
            }
            self.expect(TokenKind::RParen)?;
            break;
        }
        Ok((name, args, line))
    }

    fn parse_arg(&mut self) -> Result<Arg> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok(Arg::Ident(value)),
            TokenKind::Number(value) => Ok(Arg::Number(value)),
            TokenKind::String(value) => Ok(Arg::String(value)),
            _ => Err(anyhow!("line {}: expected argument", token.line)),
        }
    }

    fn expect_ident(&mut self, expected: &str) -> Result<()> {
        let actual = self.expect_ident_any()?;
        if normalize_ident(&actual) == expected {
            Ok(())
        } else {
            Err(self.error_here(&format!("expected {expected}")))
        }
    }

    fn expect_ident_any(&mut self) -> Result<String> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok(value),
            _ => Err(anyhow!("line {}: expected identifier", token.line)),
        }
    }

    fn expect_number_any(&mut self) -> Result<String> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Number(value) => Ok(value),
            _ => Err(anyhow!("line {}: expected number", token.line)),
        }
    }

    fn expect_comparison(&mut self) -> Result<&'static str> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Lt => Ok("<"),
            TokenKind::Le => Ok("<="),
            TokenKind::EqEq => Ok("=="),
            TokenKind::Ge => Ok(">="),
            TokenKind::Gt => Ok(">"),
            _ => Err(anyhow!("line {}: expected comparison", token.line)),
        }
    }

    fn expect_specific_comparison(&mut self, expected: &str) -> Result<()> {
        let actual = self.expect_comparison()?;
        if actual == expected {
            Ok(())
        } else {
            Err(self.error_here(&format!("expected {expected} comparison")))
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Result<()> {
        if self.consume(expected.clone())? {
            Ok(())
        } else {
            Err(self.error_here(&format!("expected {}", token_label(&expected))))
        }
    }

    fn consume(&mut self, expected: TokenKind) -> Result<bool> {
        if self.is_next(&expected) {
            self.advance();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn consume_ident(&mut self, expected: &str) -> bool {
        if self.peek_ident() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_semicolons(&mut self) {
        while self.is_next(&TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn peek_ident(&self) -> Option<&str> {
        match &self.tokens[self.pos].kind {
            TokenKind::Ident(value) => Some(value.as_str()),
            _ => None,
        }
    }

    fn is_next(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(&self.tokens[self.pos].kind) == std::mem::discriminant(expected)
    }

    fn at_eof(&self) -> bool {
        self.is_next(&TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        let index = self.pos;
        if !self.at_eof() {
            self.pos += 1;
        }
        &self.tokens[index]
    }

    fn current_line(&self) -> usize {
        self.tokens[self.pos].line
    }

    fn error_here(&self, message: &str) -> anyhow::Error {
        anyhow!("line {}: {message}", self.current_line())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tiny_function_sources() {
        assert!(is_tiny_source("fn tick() { mine(); }"));
        assert!(is_tiny_source("// xac-lang: tiny\nmine();"));
        assert!(!is_tiny_source("if output_blocked return\nmine"));
    }

    #[test]
    fn lowers_tiny_actions_to_xac_script() {
        let script = tiny_to_xac_script(
            r#"
            fn tick() {
              if (output_blocked()) { return; }
              mine();
              log("drill ready");
            }
            "#,
        )
        .unwrap();

        assert_eq!(script, "if output_blocked return\nmine\nlog drill ready");
    }
}
