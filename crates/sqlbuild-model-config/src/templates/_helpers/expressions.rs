//! The Python template expression parser.

use crate::templates::_helpers::tokens::{Token, TokenKind, tokenize};
use crate::templates::errors::TemplateError;
use crate::templates::models::{Expression, TemplateFailure};

/// Parse one `${...}` body into an expression.
pub(crate) fn parse_expression(body: &str) -> Result<Expression, TemplateFailure> {
    let tokens: Vec<Token> = tokenize(body)?;
    let mut parser = Parser { tokens, index: 0 };
    let expression = parser.expression()?;
    if parser.peek().kind == TokenKind::End {
        Ok(expression)
    } else {
        Err(parser.unexpected())
    }
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn unexpected(&self) -> TemplateFailure {
        let token = self.peek();
        TemplateFailure::Invalid(TemplateError::UnexpectedToken {
            token: token.value(),
            position: token.position,
        })
    }

    fn match_symbol(&mut self, symbol: char) -> bool {
        if self.peek().kind == TokenKind::Symbol(symbol) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expression(&mut self) -> Result<Expression, TemplateFailure> {
        match self.peek().kind.clone() {
            TokenKind::Text(text) => {
                self.index += 1;
                Ok(Expression::Text(text))
            }
            TokenKind::Interpolated { parts, .. } => {
                self.index += 1;
                Ok(Expression::Interpolated(parts))
            }
            TokenKind::Word(word) => {
                self.index += 1;
                if self.match_symbol('(') {
                    self.function_call(word)
                } else {
                    Ok(Expression::Reference(word))
                }
            }
            TokenKind::Symbol(_) | TokenKind::End => Err(self.unexpected()),
        }
    }

    fn function_call(&mut self, name: String) -> Result<Expression, TemplateFailure> {
        let mut arguments: Vec<Expression> = Vec::new();
        while self.peek().kind != TokenKind::Symbol(')') {
            arguments.push(self.expression()?);
            if !self.match_symbol(',') {
                break;
            }
        }
        if self.match_symbol(')') {
            Ok(Expression::Function { name, arguments })
        } else {
            Err(TemplateFailure::Invalid(TemplateError::ExpectedSymbol {
                symbol: ')',
                position: self.peek().position,
            }))
        }
    }
}
