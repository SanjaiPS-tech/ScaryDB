#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Word(String),
    Quoted(String),
    TypeTag(String),
    Slash,
    Semicolon,
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        
        match c {
            ';' => {
                tokens.push(Token::Semicolon);
                chars.next();
            }
            '/' => {
                tokens.push(Token::Slash);
                chars.next();
            }
            '[' => {
                chars.next(); // consume '['
                let mut tag = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c == ']' {
                        chars.next(); // consume ']'
                        break;
                    }
                    tag.push(next_c);
                    chars.next();
                }
                tokens.push(Token::TypeTag(tag));
            }
            '"' => {
                chars.next(); // consume opening quote
                let mut s = String::new();
                let mut escaped = false;
                while let Some(next_c) = chars.next() {
                    if escaped {
                        s.push(next_c);
                        escaped = false;
                    } else if next_c == '\\' {
                        escaped = true;
                    } else if next_c == '"' {
                        break;
                    } else {
                        s.push(next_c);
                    }
                }
                tokens.push(Token::Quoted(s));
            }
            _ => {
                let mut word = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_whitespace() || next_c == ';' || next_c == '/' || next_c == '[' || next_c == '"' {
                        break;
                    }
                    word.push(next_c);
                    chars.next();
                }
                if !word.is_empty() {
                    tokens.push(Token::Word(word));
                }
            }
        }
    }
    tokens
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetOp {
    pub key: String,
    pub explicit_type: Option<String>,
    pub value_str: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // DDC
    CreateDb { db_name: String },
    DropDb { db_name: String },
    Use { db_name: String },
    CreateBucket { bucket_name: String },
    DropBucket { bucket_name: String },
    ListDbs,
    ListBuckets,

    // DMC
    Set {
        bucket: String,
        ops: Vec<SetOp>,
    },
    Del {
        bucket: String,
        keys: Vec<String>,
    },

    // DRC
    Get {
        bucket: String,
        keys: Vec<String>,
    },
    Exists {
        bucket: String,
        keys: Vec<String>,
    },
    ListKeys { bucket: String },
    CountKeys { bucket: String },

    // SCC
    Boink,
    Info,
    Stats,
    Version,
    Help,
    Man,

    // CCC
    ListConfig,
    GetConfig { property: String },
    SetConfig { property: String, value: String },
}

pub fn parse_command(input: &str) -> Result<Command, String> {
    let tokens = tokenize(input);
    if tokens.is_empty() {
        return Err("No command provided".to_string());
    }

    let mut cursor = 0;
    
    // Get command verb
    let verb = match &tokens[cursor] {
        Token::Word(w) => w.to_uppercase(),
        Token::Semicolon => return Err("Unexpected semicolon".to_string()),
        other => return Err(format!("Unexpected token: {:?}", other)),
    };
    cursor += 1;

    match verb.as_str() {
        "CREATE" => {
            if cursor >= tokens.len() {
                return Err("Expected DB or BUCKET after CREATE".to_string());
            }
            let sub_verb = expect_word(&tokens[cursor])?.to_uppercase();
            cursor += 1;

            match sub_verb.as_str() {
                "DB" => {
                    let db_name = expect_word(get_token(&tokens, &mut cursor)?)?;
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::CreateDb { db_name })
                }
                "BUCKET" => {
                    let bucket_name = expect_word(get_token(&tokens, &mut cursor)?)?;
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::CreateBucket { bucket_name })
                }
                other => Err(format!("Expected DB or BUCKET, found '{}'", other)),
            }
        }
        "DROP" => {
            if cursor >= tokens.len() {
                return Err("Expected DB or BUCKET after DROP".to_string());
            }
            let sub_verb = expect_word(&tokens[cursor])?.to_uppercase();
            cursor += 1;

            match sub_verb.as_str() {
                "DB" => {
                    let db_name = expect_word(get_token(&tokens, &mut cursor)?)?;
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::DropDb { db_name })
                }
                "BUCKET" => {
                    let bucket_name = expect_word(get_token(&tokens, &mut cursor)?)?;
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::DropBucket { bucket_name })
                }
                other => Err(format!("Expected DB or BUCKET, found '{}'", other)),
            }
        }
        "USE" => {
            let db_name = expect_word(get_token(&tokens, &mut cursor)?)?;
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Use { db_name })
        }
        "LIST" => {
            if cursor >= tokens.len() {
                return Err("Expected list target (DBS, BUCKETS, CONFIG, or bucket name)".to_string());
            }
            let target = expect_word(&tokens[cursor])?;
            cursor += 1;

            match target.to_uppercase().as_str() {
                "DBS" | "DATABASES" => {
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::ListDbs)
                }
                "BUCKETS" | "BUCK" => {
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::ListBuckets)
                }
                "CONFIG" => {
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::ListConfig)
                }
                _ => {
                    consume_optional_semicolon(&tokens, &mut cursor);
                    Ok(Command::ListKeys { bucket: target })
                }
            }
        }
        "COUNT" => {
            let bucket = expect_word(get_token(&tokens, &mut cursor)?)?;
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::CountKeys { bucket })
        }
        "SET" => {
            if cursor >= tokens.len() {
                return Err("Expected bucket or CONFIG after SET".to_string());
            }
            let target = expect_word(&tokens[cursor])?;
            cursor += 1;

            if target.to_uppercase() == "CONFIG" {
                let property = expect_word(get_token(&tokens, &mut cursor)?)?;
                let value = match get_token(&tokens, &mut cursor)? {
                    Token::Word(w) => w.clone(),
                    Token::Quoted(q) => q.clone(),
                    other => return Err(format!("Expected config value, found {:?}", other)),
                };
                consume_optional_semicolon(&tokens, &mut cursor);
                Ok(Command::SetConfig { property, value })
            } else {
                // Parse SET ops separated by Slash
                let mut ops = Vec::new();
                loop {
                    let key = expect_word(get_token(&tokens, &mut cursor)?)?;
                    let mut explicit_type = None;
                    if let Some(Token::TypeTag(t)) = tokens.get(cursor) {
                        explicit_type = Some(t.clone());
                        cursor += 1;
                    }
                    let val_tok = get_token(&tokens, &mut cursor)?;
                    let value_str = match val_tok {
                        Token::Word(w) => w.clone(),
                        Token::Quoted(q) => format!("\"{}\"", q), // preserve quotes for Value::parse
                        other => return Err(format!("Expected value, found {:?}", other)),
                    };
                    
                    ops.push(SetOp {
                        key,
                        explicit_type,
                        value_str,
                    });

                    // Check if there are more operations
                    if let Some(Token::Slash) = tokens.get(cursor) {
                        cursor += 1;
                    } else {
                        break;
                    }
                }
                consume_optional_semicolon(&tokens, &mut cursor);
                Ok(Command::Set { bucket: target, ops })
            }
        }
        "GET" => {
            if cursor >= tokens.len() {
                return Err("Expected bucket or CONFIG after GET".to_string());
            }
            let target = expect_word(&tokens[cursor])?;
            cursor += 1;

            if target.to_uppercase() == "CONFIG" {
                let property = expect_word(get_token(&tokens, &mut cursor)?)?;
                consume_optional_semicolon(&tokens, &mut cursor);
                Ok(Command::GetConfig { property })
            } else {
                let mut keys = Vec::new();
                loop {
                    let key = expect_word(get_token(&tokens, &mut cursor)?)?;
                    keys.push(key);
                    if let Some(Token::Slash) = tokens.get(cursor) {
                        cursor += 1;
                    } else {
                        break;
                    }
                }
                consume_optional_semicolon(&tokens, &mut cursor);
                Ok(Command::Get { bucket: target, keys })
            }
        }
        "DEL" => {
            let bucket = expect_word(get_token(&tokens, &mut cursor)?)?;
            let mut keys = Vec::new();
            loop {
                let key = expect_word(get_token(&tokens, &mut cursor)?)?;
                keys.push(key);
                if let Some(Token::Slash) = tokens.get(cursor) {
                    cursor += 1;
                } else {
                    break;
                }
            }
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Del { bucket, keys })
        }
        "EXISTS" => {
            let bucket = expect_word(get_token(&tokens, &mut cursor)?)?;
            let mut keys = Vec::new();
            loop {
                let key = expect_word(get_token(&tokens, &mut cursor)?)?;
                keys.push(key);
                if let Some(Token::Slash) = tokens.get(cursor) {
                    cursor += 1;
                } else {
                    break;
                }
            }
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Exists { bucket, keys })
        }
        "BOINK" => {
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Boink)
        }
        "INFO" => {
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Info)
        }
        "STATS" => {
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Stats)
        }
        "VERSION" => {
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Version)
        }
        "HELP" => {
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Help)
        }
        "MAN" => {
            consume_optional_semicolon(&tokens, &mut cursor);
            Ok(Command::Man)
        }
        other => Err(format!("Unknown command verb: '{}'", other)),
    }
}

fn get_token<'a>(tokens: &'a [Token], cursor: &mut usize) -> Result<&'a Token, String> {
    if *cursor >= tokens.len() {
        return Err("Unexpected end of command input".to_string());
    }
    let tok = &tokens[*cursor];
    *cursor += 1;
    Ok(tok)
}

fn expect_word(tok: &Token) -> Result<String, String> {
    match tok {
        Token::Word(w) => Ok(w.clone()),
        other => Err(format!("Expected identifier word, found {:?}", other)),
    }
}

fn consume_optional_semicolon(tokens: &[Token], cursor: &mut usize) {
    if let Some(Token::Semicolon) = tokens.get(*cursor) {
        *cursor += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer() {
        let tokens = tokenize("SET users u1 \"Alice\" / u2 [INT] 42;");
        assert_eq!(
            tokens,
            vec![
                Token::Word("SET".to_string()),
                Token::Word("users".to_string()),
                Token::Word("u1".to_string()),
                Token::Quoted("Alice".to_string()),
                Token::Slash,
                Token::Word("u2".to_string()),
                Token::TypeTag("INT".to_string()),
                Token::Word("42".to_string()),
                Token::Semicolon,
            ]
        );
    }

    #[test]
    fn test_parse_create_db() {
        let cmd = parse_command("CREATE DB sales;");
        assert_eq!(cmd, Ok(Command::CreateDb { db_name: "sales".to_string() }));
    }

    #[test]
    fn test_parse_set() {
        let cmd = parse_command("SET users u1 \"Alice\" / u2 [INT] 42;");
        assert_eq!(
            cmd,
            Ok(Command::Set {
                bucket: "users".to_string(),
                ops: vec![
                    SetOp {
                        key: "u1".to_string(),
                        explicit_type: None,
                        value_str: "\"Alice\"".to_string(),
                    },
                    SetOp {
                        key: "u2".to_string(),
                        explicit_type: Some("INT".to_string()),
                        value_str: "42".to_string(),
                    }
                ]
            })
        );
    }

    #[test]
    fn test_parse_get() {
        let cmd = parse_command("GET users u1 / u2;");
        assert_eq!(
            cmd,
            Ok(Command::Get {
                bucket: "users".to_string(),
                keys: vec!["u1".to_string(), "u2".to_string()]
            })
        );
    }
}
