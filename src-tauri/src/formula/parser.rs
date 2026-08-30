// Parser Airtable-like -> AST. Pas de `eval` JS : AST évaluée en Rust.

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Str(String),
    Field(String),
    Ident(String),
    LParen,
    RParen,
    Comma,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Amp,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Concat,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    Not,
    Neg,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    Str(String),
    Bool(bool),
    Null,
    Field(String),
    Call(String, Vec<Expr>),
    Unary(Op, Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '/' => {
                chars.next();
                tokens.push(Token::Slash);
            }
            '%' => {
                chars.next();
                tokens.push(Token::Percent);
            }
            '&' => {
                chars.next();
                tokens.push(Token::Amp);
            }
            '=' => {
                chars.next();
                tokens.push(Token::Eq);
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Neq);
                } else {
                    return Err("caractère inattendu '!'".into());
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Lte);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Gte);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '{' => {
                chars.next();
                let mut name = String::new();
                while let Some(&c2) = chars.peek() {
                    if c2 == '}' {
                        break;
                    }
                    name.push(c2);
                    chars.next();
                }
                if chars.peek() != Some(&'}') {
                    return Err("champ non fermé '{...}'".into());
                }
                chars.next();
                tokens.push(Token::Field(name.trim().to_string()));
            }
            '\'' | '"' => {
                let quote = c;
                chars.next();
                let mut s = String::new();
                while let Some(&c2) = chars.peek() {
                    if c2 == quote {
                        break;
                    }
                    if c2 == '\\' {
                        chars.next();
                        if let Some(&e) = chars.peek() {
                            s.push(e);
                            chars.next();
                        }
                    } else {
                        s.push(c2);
                        chars.next();
                    }
                }
                if chars.peek() != Some(&quote) {
                    return Err("chaîne non fermée".into());
                }
                chars.next();
                tokens.push(Token::Str(s));
            }
            c if c.is_ascii_digit() || c == '.' => {
                let mut num = String::new();
                while let Some(&c2) = chars.peek() {
                    if c2.is_ascii_digit() || c2 == '.' {
                        num.push(c2);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let n: f64 = num
                    .parse()
                    .map_err(|_| format!("nombre invalide '{num}'"))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut ident = String::new();
                while let Some(&c2) = chars.peek() {
                    if c2.is_ascii_alphanumeric() || c2 == '_' {
                        ident.push(c2);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(ident));
            }
            other => return Err(format!("caractère inattendu '{other}'")),
        }
    }
    tokens.push(Token::Eof);
    Ok(tokens)
}

pub fn parse(input: &str) -> Result<Expr, String> {
    let tokens = tokenize(input)?;
    let mut p = Parser { tokens, pos: 0 };
    let e = p.parse_expr()?;
    if !matches!(p.peek(), Token::Eof) {
        return Err(format!("token inattendu en fin d'expression : {:?}", p.peek()));
    }
    Ok(e)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, t: &Token) -> Result<(), String> {
        if self.peek() == t {
            self.advance();
            Ok(())
        } else {
            Err(format!("attendu {t:?}, trouvé {:?}", self.peek()))
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_concat()
    }

    fn parse_concat(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_compare()?;
        while matches!(self.peek(), Token::Amp) {
            self.advance();
            let right = self.parse_compare()?;
            left = Expr::Binary(Op::Concat, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_compare(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Token::Eq => Op::Eq,
                Token::Neq => Op::Neq,
                Token::Lt => Op::Lt,
                Token::Gt => Op::Gt,
                Token::Lte => Op::Lte,
                Token::Gte => Op::Gte,
                _ => break,
            };
            self.advance();
            let right = self.parse_add()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus => Op::Add,
                Token::Minus => Op::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => Op::Mul,
                Token::Slash => Op::Div,
                Token::Percent => Op::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Token::Minus => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::Unary(Op::Neg, Box::new(e)))
            }
            Token::Ident(id) if id == "NOT" => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::Unary(Op::Not, Box::new(e)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Token::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Token::Field(name) => {
                self.advance();
                Ok(Expr::Field(name))
            }
            Token::LParen => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(e)
            }
            Token::Ident(id) => {
                self.advance();
                match id.as_str() {
                    "TRUE" => Ok(Expr::Bool(true)),
                    "FALSE" => Ok(Expr::Bool(false)),
                    "NULL" | "BLANK" => Ok(Expr::Null),
                    _ => {
                        self.expect(&Token::LParen)?;
                        let mut args = Vec::new();
                        if !matches!(self.peek(), Token::RParen) {
                            loop {
                                args.push(self.parse_expr()?);
                                if matches!(self.peek(), Token::Comma) {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                        }
                        self.expect(&Token::RParen)?;
                        Ok(Expr::Call(id, args))
                    }
                }
            }
            other => Err(format!("expression inattendue : {other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Expr, parse};

    #[test]
    fn parses_basic() {
        let e = parse("IF({Status}='Done', 1, 0)").unwrap();
        assert!(matches!(e, Expr::Call(..)));
        let e = parse("CONCATENATE('a', {Nom}, '-', {Montant})").unwrap();
        assert!(matches!(e, Expr::Call(..)));
        let e = parse("({Montant} + 10) * 2 > 50").unwrap();
        assert!(matches!(e, Expr::Binary(..)));
    }

    #[test]
    fn rejects_bad() {
        assert!(parse("IF(1)").is_ok());
        assert!(parse("(1 + 2").is_err());
        assert!(parse("{Nom").is_err());
    }
}
