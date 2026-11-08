//! SQLBuild's own type reader, which builds the tree SQLQuack's type parsers build.

use serde_json::{Map, Value, json};

use crate::type_system::_helpers::spelling::SpellingDialect;

/// The type tree of `type_sql`, or why the reader rejects it.
pub(crate) fn read_type(type_sql: &str, dialect: SpellingDialect) -> Result<Value, String> {
    let tokens: Vec<Token> = tokens(type_sql, dialect)?;
    let mut reader = Reader {
        tokens: &tokens,
        position: 0,
        dialect,
    };
    let tree: Value = reader.data_type()?;
    if reader.peek() == Some(&Token::Punct(';')) {
        reader.position += 1;
    }
    match reader.peek() {
        None => Ok(tree),
        Some(token) => Err(format!("unexpected {} after the type", token.text())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Quoted(String),
    /// A single-quoted string, such as a ClickHouse enum member or time zone.
    Literal(String),
    Number(String),
    Punct(char),
}

impl Token {
    fn text(&self) -> String {
        match self {
            Self::Word(text) | Self::Quoted(text) | Self::Literal(text) | Self::Number(text) => {
                format!("'{text}'")
            }
            Self::Punct(character) => format!("'{character}'"),
        }
    }

    /// The token as a type name writes it.
    fn source(&self) -> String {
        match self {
            Self::Word(text) | Self::Number(text) => text.clone(),
            Self::Quoted(text) => format!("\"{text}\""),
            Self::Literal(text) => format!("'{}'", text.replace('\'', "''")),
            Self::Punct(character) => character.to_string(),
        }
    }
}

fn tokens(type_sql: &str, dialect: SpellingDialect) -> Result<Vec<Token>, String> {
    let characters: Vec<char> = type_sql.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut index: usize = 0;
    while let Some(&character) = characters.get(index) {
        if character.is_whitespace() {
            index += 1;
        } else if character.is_alphabetic() || character == '_' {
            let start: usize = index;
            while characters
                .get(index)
                .is_some_and(|next| next.is_alphanumeric() || *next == '_' || *next == '$')
            {
                index += 1;
            }
            tokens.push(Token::Word(characters[start..index].iter().collect()));
        } else if character.is_ascii_digit() {
            let start: usize = index;
            while characters.get(index).is_some_and(char::is_ascii_digit) {
                index += 1;
            }
            if characters
                .get(index)
                .is_some_and(|next| next.is_alphabetic() || *next == '_')
            {
                return Err(format!("invalid identifier at offset {start}"));
            }
            tokens.push(Token::Number(characters[start..index].iter().collect()));
        } else if let Some(close) = quote_close(character, dialect) {
            let start: usize = index + 1;
            index = start;
            while characters.get(index).is_some_and(|next| *next != close) {
                index += 1;
            }
            if index >= characters.len() {
                return Err("unterminated quoted name".to_owned());
            }
            tokens.push(Token::Quoted(characters[start..index].iter().collect()));
            index += 1;
        } else if character == '\'' && dialect == SpellingDialect::ClickHouse {
            let (literal, end) = literal(&characters, index)?;
            tokens.push(Token::Literal(literal));
            index = end;
        } else if "()<>,[]:;".contains(character)
            || ("=-".contains(character) && dialect == SpellingDialect::ClickHouse)
        {
            tokens.push(Token::Punct(character));
            index += 1;
        } else {
            return Err(format!("unexpected character '{character}'"));
        }
    }
    Ok(tokens)
}

/// The single-quoted string opening at `start`, with `''` and backslash escapes, and the index after it.
fn literal(characters: &[char], start: usize) -> Result<(String, usize), String> {
    let mut text: String = String::new();
    let mut index: usize = start + 1;
    loop {
        match characters.get(index) {
            None => return Err("unterminated string".to_owned()),
            Some('\\') => {
                text.extend(characters.get(index + 1));
                index += 2;
            }
            Some('\'') if characters.get(index + 1) == Some(&'\'') => {
                text.push('\'');
                index += 2;
            }
            Some('\'') => return Ok((text, index + 1)),
            Some(&character) => {
                text.push(character);
                index += 1;
            }
        }
    }
}

/// The closing quote of a quoted name `open` starts in `dialect`.
fn quote_close(open: char, dialect: SpellingDialect) -> Option<char> {
    let backtick: bool = matches!(
        dialect,
        SpellingDialect::Databricks | SpellingDialect::BigQuery | SpellingDialect::ClickHouse
    );
    match open {
        '`' if backtick => Some('`'),
        '"' if !matches!(
            dialect,
            SpellingDialect::Databricks | SpellingDialect::BigQuery
        ) =>
        {
            Some('"')
        }
        '[' if dialect == SpellingDialect::Tsql => Some(']'),
        _ => None,
    }
}

struct Reader<'a> {
    tokens: &'a [Token],
    position: usize,
    dialect: SpellingDialect,
}

impl Reader<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<Token> {
        let token: Option<Token> = self.peek().cloned();
        self.position += usize::from(token.is_some());
        token
    }

    fn eat_punct(&mut self, character: char) -> bool {
        let found: bool = self.peek() == Some(&Token::Punct(character));
        self.position += usize::from(found);
        found
    }

    fn expect_punct(&mut self, character: char) -> Result<(), String> {
        if self.eat_punct(character) {
            return Ok(());
        }
        Err(match self.peek() {
            Some(token) => format!("expected '{character}', found {}", token.text()),
            None => format!("expected '{character}' at the end of the type"),
        })
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let found: bool =
            matches!(self.peek(), Some(Token::Word(text)) if text.eq_ignore_ascii_case(word));
        self.position += usize::from(found);
        found
    }

    fn data_type(&mut self) -> Result<Value, String> {
        let mut tree: Value = self.base_type()?;
        while self.eat_punct('[') {
            let dimension: Option<u32> = match self.peek() {
                Some(Token::Number(_)) => Some(self.number()?),
                _ => None,
            };
            self.expect_punct(']')?;
            let mut array: Map<String, Value> = Map::new();
            array.insert("data_type".to_owned(), json!("array"));
            array.insert("element_type".to_owned(), tree);
            if let Some(dimension) = dimension {
                array.insert("dimension".to_owned(), json!(dimension));
            }
            tree = Value::Object(array);
        }
        Ok(tree)
    }

    fn base_type(&mut self) -> Result<Value, String> {
        match self.next() {
            Some(Token::Quoted(name)) => self.custom(name),
            Some(Token::Word(word)) => self.named_type(&word),
            other => Err(unexpected("a type name", other.as_ref())),
        }
    }

    fn named_type(&mut self, word: &str) -> Result<Value, String> {
        let upper: String = word.to_ascii_uppercase();
        match upper.as_str() {
            "ARRAY" => {
                let close: char = self.open_nested()?;
                let element: Value = self.data_type()?;
                self.expect_punct(close)?;
                Ok(json!({"data_type": "array", "element_type": element}))
            }
            "MAP" => {
                let close: char = self.open_nested()?;
                let key: Value = self.data_type()?;
                self.expect_punct(',')?;
                let value: Value = self.data_type()?;
                self.expect_punct(close)?;
                Ok(json!({"data_type": "map", "key_type": key, "value_type": value}))
            }
            "STRUCT" => {
                let close: char = self.open_nested()?;
                let mut fields: Vec<Value> = Vec::new();
                loop {
                    let name: String = match self.next() {
                        Some(Token::Word(name) | Token::Quoted(name)) => name,
                        _ => return Err("expected a struct field name".to_owned()),
                    };
                    self.eat_punct(':');
                    let data_type: Value = self.data_type()?;
                    fields.push(json!({"name": name, "data_type": data_type}));
                    if !self.eat_punct(',') {
                        break;
                    }
                }
                self.expect_punct(close)?;
                Ok(json!({"data_type": "struct", "fields": fields, "nested": close == ')'}))
            }
            "DOUBLE" => {
                self.eat_word("PRECISION");
                let (precision, scale) = self.precision_scale()?;
                Ok(json!({"data_type": "double", "precision": precision, "scale": scale}))
            }
            "CHARACTER" | "CHAR" if self.eat_word("VARYING") => {
                let length: Option<u32> = self.text_length(Self::length)?;
                Ok(json!({"data_type": "var_char", "length": length}))
            }
            "TIMESTAMP" => self.time_type("timestamp", self.dialect == SpellingDialect::BigQuery),
            "TIME" => self.time_type("time", false),
            _ => self.scalar_type(word, &upper),
        }
    }

    /// A time or timestamp; `zoned` when the bare type has a time zone, as in BigQuery.
    fn time_type(&mut self, tag: &str, zoned: bool) -> Result<Value, String> {
        let precision: Option<u32> = self.length()?;
        let timezone: bool = self.time_zone()?.unwrap_or(zoned);
        Ok(json!({"data_type": tag, "precision": precision, "timezone": timezone}))
    }

    /// The opening bracket of a nested type's parameters, returning its closing bracket.
    fn open_nested(&mut self) -> Result<char, String> {
        if self.eat_punct('<') {
            return Ok('>');
        }
        self.expect_punct('(')?;
        Ok(')')
    }

    /// `WITH TIME ZONE` or `WITHOUT TIME ZONE` after a time type, None when neither follows.
    fn time_zone(&mut self) -> Result<Option<bool>, String> {
        let timezone: bool = if self.eat_word("WITH") {
            true
        } else if self.eat_word("WITHOUT") {
            false
        } else {
            return Ok(None);
        };
        if self.eat_word("TIME") && self.eat_word("ZONE") {
            return Ok(Some(timezone));
        }
        Err("expected TIME ZONE".to_owned())
    }

    fn scalar_type(&mut self, word: &str, upper: &str) -> Result<Value, String> {
        let tagged = |tag: &str, length: Option<u32>| json!({"data_type": tag, "length": length});
        Ok(match upper {
            "INT" | "INTEGER" => tagged("int", self.sized_length()?),
            "BIGINT" => tagged("big_int", self.sized_length()?),
            "SMALLINT" => tagged("small_int", self.sized_length()?),
            "TINYINT" => tagged("tiny_int", self.sized_length()?),
            "VARCHAR" => {
                if self.max_length() {
                    return Ok(json!({"data_type": "custom", "name": upper}));
                }
                tagged("var_char", self.text_length(Self::sized_length)?)
            }
            "CHAR" | "CHARACTER" => tagged("char", self.text_length(Self::sized_length)?),
            "STRING" => tagged("string", self.text_length(Self::length)?),
            "BINARY" => tagged("binary", self.length()?),
            "GEOGRAPHY" if self.single_geography() => {
                if !self.max_length() {
                    self.length()?;
                }
                json!({"data_type": "custom", "name": upper})
            }
            "VARBINARY" | "NVARCHAR" | "GEOGRAPHY" if self.max_length() => {
                json!({"data_type": "custom", "name": format!("{upper}(MAX)")})
            }
            "VARBINARY" => tagged("var_binary", self.length()?),
            "TEXT" | "NTEXT" => match self.length()? {
                Some(length) => json!({"data_type": "text_with_length", "length": length}),
                None => json!({"data_type": "text"}),
            },
            "DECIMAL" | "NUMERIC" => {
                let (precision, scale) = self.precision_scale()?;
                json!({"data_type": "decimal", "precision": precision, "scale": scale})
            }
            "FLOAT" => self.float_type(false)?,
            "REAL" => self.float_type(true)?,
            "TIMESTAMPTZ" | "TIMESTAMP_TZ" => {
                let precision: Option<u32> = self.length()?;
                json!({"data_type": "timestamp", "precision": precision, "timezone": true})
            }
            "DATETIME" if self.max_length() => json!({"data_type": "custom", "name": upper}),
            "TIMESTAMPNTZ" | "TIMESTAMP_NTZ" | "TIMESTAMPLTZ" | "TIMESTAMP_LTZ" | "DATETIME" => {
                let _precision: Option<u32> = self.length()?;
                json!({"data_type": "custom", "name": upper.replace('_', "")})
            }
            "BOOL" | "BOOLEAN" => json!({"data_type": "boolean"}),
            "DATE" => json!({"data_type": "date"}),
            "INTERVAL" => json!({"data_type": "interval"}),
            "JSON" => json!({"data_type": "json"}),
            "JSONB" => json!({"data_type": "json_b"}),
            "UUID" => json!({"data_type": "uuid"}),
            "BLOB" => json!({"data_type": "blob"}),
            "HUGEINT" => json!({"data_type": "int128"}),
            "LONG" => json!({"data_type": "custom", "name": upper}),
            "NVARCHAR" | "GEOGRAPHY" => {
                let length: Option<u32> = self.length()?;
                let name: String = match length {
                    Some(length) => format!("{upper}({length})"),
                    None => upper.to_owned(),
                };
                json!({"data_type": "custom", "name": name})
            }
            _ => return self.custom(word.to_owned()),
        })
    }

    /// Whether the dialect has one GEOGRAPHY, which takes no parameters: BigQuery and Snowflake.
    fn single_geography(&self) -> bool {
        matches!(
            self.dialect,
            SpellingDialect::BigQuery | SpellingDialect::Snowflake
        )
    }

    /// A FLOAT or REAL; Postgres's REAL is a 4-byte float, whatever precision follows it.
    fn float_type(&mut self, real: bool) -> Result<Value, String> {
        let (mut precision, mut scale) = self.precision_scale()?;
        if real && self.dialect == SpellingDialect::Postgres {
            (precision, scale) = (None, None);
        }
        Ok(json!({
            "data_type": "float",
            "precision": precision,
            "scale": scale,
            "real_spelling": real,
        }))
    }

    /// Any other type name with any number of integer parameters, kept as written.
    fn custom(&mut self, name: String) -> Result<Value, String> {
        if !self.eat_punct('(') {
            return Ok(json!({"data_type": "custom", "name": name}));
        }
        if self.eat_punct(')') {
            return Ok(json!({"data_type": "custom", "name": format!("{name}()")}));
        }
        let mut parameters: Vec<String> = Vec::new();
        loop {
            match self.dialect {
                SpellingDialect::ClickHouse => parameters.push(self.clickhouse_parameter(&name)?),
                _ => match self.next() {
                    Some(Token::Number(number)) => parameters.push(number),
                    Some(Token::Word(word)) => parameters.push(word),
                    _ => return Err(format!("expected a parameter of {name}")),
                },
            }
            if !self.eat_punct(',') {
                break;
            }
        }
        self.expect_punct(')')?;
        Ok(json!({"data_type": "custom", "name": format!("{name}({})", parameters.join(", "))}))
    }

    /// One ClickHouse type parameter as written: a nested type, as in `Nullable(DateTime64(3))`,
    /// a named tuple field, or an enum member and its value.
    fn clickhouse_parameter(&mut self, name: &str) -> Result<String, String> {
        let mut text: String = String::new();
        let mut depth: usize = 0;
        let mut spaced: bool = false;
        loop {
            let token: Token = match self.peek() {
                None => return Err(format!("expected ')' after the parameters of {name}")),
                Some(Token::Punct(',' | ')')) if depth == 0 => break,
                Some(token) => token.clone(),
            };
            self.position += 1;
            let word: bool = !matches!(token, Token::Punct(_));
            match token {
                Token::Punct('(') => depth += 1,
                Token::Punct(')') => depth -= 1,
                _ => {}
            }
            if token == Token::Punct('=') {
                text.push_str(" = ");
                spaced = false;
                continue;
            }
            if word && spaced {
                text.push(' ');
            }
            text.push_str(&token.source());
            spaced = word;
        }
        if text.is_empty() {
            return Err(format!("expected a parameter of {name}"));
        }
        Ok(text)
    }

    /// The optional length of an integer or character type; empty parentheses state none.
    fn sized_length(&mut self) -> Result<Option<u32>, String> {
        if self.peek() == Some(&Token::Punct('('))
            && self.tokens.get(self.position + 1) == Some(&Token::Punct(')'))
        {
            self.position += 2;
            return Ok(None);
        }
        self.length()
    }

    /// A character type's length, which ClickHouse reads and drops: its `String` has none.
    fn text_length(
        &mut self,
        read: fn(&mut Self) -> Result<Option<u32>, String>,
    ) -> Result<Option<u32>, String> {
        let length: Option<u32> = read(self)?;
        Ok(length.filter(|_| self.dialect != SpellingDialect::ClickHouse))
    }

    /// A parenthesized `MAX`, which only the variable-length types take.
    fn max_length(&mut self) -> bool {
        let found: bool = self.peek() == Some(&Token::Punct('('))
            && matches!(self.tokens.get(self.position + 1), Some(Token::Word(word)) if word.eq_ignore_ascii_case("MAX"))
            && self.tokens.get(self.position + 2) == Some(&Token::Punct(')'));
        self.position += if found { 3 } else { 0 };
        found
    }

    /// One optional parenthesized integer.
    fn length(&mut self) -> Result<Option<u32>, String> {
        if !self.eat_punct('(') {
            return Ok(None);
        }
        let length: u32 = self.number()?;
        self.expect_punct(')')?;
        Ok(Some(length))
    }

    /// An optional parenthesized precision and scale.
    fn precision_scale(&mut self) -> Result<(Option<u32>, Option<u32>), String> {
        if !self.eat_punct('(') {
            return Ok((None, None));
        }
        let precision: u32 = self.number()?;
        let scale: Option<u32> = if self.eat_punct(',') {
            Some(self.number()?)
        } else {
            None
        };
        self.expect_punct(')')?;
        Ok((Some(precision), scale))
    }

    fn number(&mut self) -> Result<u32, String> {
        match self.next() {
            Some(Token::Number(number)) => number
                .parse::<u32>()
                .map_err(|_| format!("invalid number: {number}")),
            other => Err(unexpected("a number", other.as_ref())),
        }
    }
}

/// Why `found`, or the end of the type, is not the `expected` token.
fn unexpected(expected: &str, found: Option<&Token>) -> String {
    match found {
        Some(token) => format!("expected {expected}, found {}", token.text()),
        None => format!("expected {expected}"),
    }
}
