use std::iter::Peekable;

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum TokType {
    // An unquoted string, e.g. `.config/`.
    Str(String),
    // "Paren" refers to parentheses: `(` and `)`.
    LParen,
    RParen,
    // "Brace" refers to curly braces: `{` and `}`.
    LBrace,
    RBrace,
    // "Bracket" refers to square brackets: `[` and `]`.
    LBracket,
    RBracket,
    // The mapping operator, `=>`.
    MapsTo,
    Comma,
    Colon,
    Semicolon,
}
impl TokType {
    pub fn unwrap_str(self) -> String {
        match self {
            TokType::Str(s) => s,
            _ => panic!("Failed to unwrap str"),
        }
    }
}

pub const EXPECTED_STR: &[TokType; 1] = &[TokType::Str(String::new())];

impl<'a> From<&'a str> for TokType {
    fn from(s: &'a str) -> TokType {
        TokType::Str(s.to_owned())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Token {
    pub toktype: TokType,
    pub line: usize,
}

impl Token {
    pub fn new(toktype: TokType, line: usize) -> Self {
        Self { toktype, line }
    }
    pub fn string(s: &str, line: usize) -> Self {
        Self {
            toktype: TokType::Str(s.to_owned()),
            line,
        }
    }
}

pub struct Lexer<I: Iterator<Item = char>> {
    iter: Peekable<I>,
    line: usize,
}

impl<I: Iterator<Item = char>> Lexer<I> {
    pub fn new(iter: Peekable<I>) -> Lexer<I> {
        Lexer { iter, line: 1 }
    }
}

fn get_processed_string<I: Iterator<Item = char>>(iter: &mut Peekable<I>, start: char) -> String {
    let is_ending_char = |c: char| -> bool {
        c.is_ascii_whitespace()
            || ['(', ')', '{', '}', '[', ']', ',', ';', ':', '=']
                .iter()
                .any(|e| *e == c)
    };
    let mut ret = start.to_string();
    loop {
        if iter.peek().map(|&c| c == '\\').unwrap_or(false) {
            iter.next();
            let next_char = iter.peek().cloned();
            match next_char {
                Some('*') | Some('?') | None => {
                    ret.push('\\');
                }
                _ => {}
            }
            if let Some(c) = next_char {
                // Push the character if it exists.
                ret.push(c);
            }
            // Unconditionally advance the iterator.
            iter.next();
        } else if iter.peek().map(|&c| !is_ending_char(c)).unwrap_or(false) {
            ret.push(iter.next().unwrap());
        } else {
            break;
        }
    }
    ret
}

impl<I: Iterator<Item = char>> Iterator for Lexer<I> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        macro_rules! new_tok {
            ($t:ident) => {
                Token::new(TokType::$t, self.line)
            };
        }

        loop {
            match self.iter.next() {
                None => return None,
                Some(chr) => match chr {
                    '\n' => self.line += 1,
                    '(' => return Some(new_tok!(LParen)),
                    ')' => return Some(new_tok!(RParen)),
                    '{' => return Some(new_tok!(LBrace)),
                    '}' => return Some(new_tok!(RBrace)),
                    '[' => return Some(new_tok!(LBracket)),
                    ']' => return Some(new_tok!(RBracket)),
                    ',' => return Some(new_tok!(Comma)),
                    ';' => return Some(new_tok!(Semicolon)),
                    ':' => return Some(new_tok!(Colon)),
                    '=' => {
                        if self.iter.peek().map(|x| *x == '>').unwrap_or(false) {
                            self.iter.next();
                            return Some(new_tok!(MapsTo));
                        } else {
                            return Some(Token::string(
                                &get_processed_string(&mut self.iter, '='),
                                self.line,
                            ));
                        }
                    }
                    ' ' | '\t' | '\r' => {}
                    _ => {
                        return Some(Token::string(
                            &get_processed_string(&mut self.iter, chr),
                            self.line,
                        ))
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_lexer_output(input: &str, expected: Vec<Token>) {
        let chars = input.chars().peekable();
        let lex = Lexer::new(chars);
        lex.zip(expected.iter())
            .enumerate()
            .for_each(|(idx, (out, ex_out))| {
                assert!(
                    out == *ex_out,
                    "Not equal at position {}:\n`{:?}`\n!=\n`{:?}`",
                    idx,
                    out,
                    ex_out
                );
            });
    }

    macro_rules! tok {
        ($t:ident, $l:literal) => {
            Token::new(TokType::$t, $l)
        };
        ($s:tt, $l:literal) => {
            Token::string($s, $l)
        };
    }

    #[test]
    fn ignore_pattern_chars_in_processed_string() {
        // '*' and '?' are pattern chars. They should be ignored if the user tries to escape them.
        // These characters should be handled later with patmatch.
        let proc_str = get_processed_string(&mut "\\[\\]\\*\\?".to_owned().chars().peekable(), '[');
        assert_eq!(proc_str, "[[]\\*\\?");
    }

    #[test]
    fn full_example_config() {
        check_lexer_output(
            "\
~/.config/nvim/init.vim => config.nvim;
~/{
    os(linux, macos): _config,
    default: .config
}/rofi.rasi;
/etc/fonts/local.conf => local.conf;
",
            vec![
                tok!("~/.config/nvim/init.vim", 1),
                tok!(MapsTo, 1),
                tok!("config.nvim", 1),
                tok!(Semicolon, 1),
                tok!("~/", 2),
                tok!(LBrace, 2),
                tok!("os", 3),
                tok!(LParen, 3),
                tok!("linux", 3),
                tok!(Comma, 3),
                tok!("macos", 3),
                tok!(RParen, 3),
                tok!(Colon, 3),
                tok!("_config", 3),
                tok!(Comma, 3),
                tok!("default", 4),
                tok!(Colon, 4),
                tok!(".config", 4),
                tok!(RBrace, 5),
                tok!("/rofi.rasi", 5),
                tok!(Semicolon, 5),
                tok!("/etc/fonts/local.conf", 6),
                tok!(MapsTo, 6),
                tok!("local.conf", 6),
                tok!(Semicolon, 6),
            ],
        );
    }

    #[test]
    fn single_statement() {
        check_lexer_output(
            "/etc/conf.d/minecraft => ~/.mc.conf;",
            vec![
                tok!("/etc/conf.d/minecraft", 1),
                tok!(MapsTo, 1),
                tok!("~/.mc.conf", 1),
                tok!(Semicolon, 1),
            ],
        );
    }

    #[test]
    fn excessive_whitespace() {
        check_lexer_output(
            "check\t\r\n\r\r            \nq",
            vec![tok!("check", 1), tok!("q", 3)],
        );
    }

    #[test]
    fn all_symbols() {
        check_lexer_output(
            "(  \t){ }\n [ ]\n ; \n =>\t\n = >\n ,\n",
            vec![
                tok!(LParen, 1),
                tok!(RParen, 1),
                tok!(LBrace, 1),
                tok!(RBrace, 1),
                tok!(LBracket, 2),
                tok!(RBracket, 2),
                tok!(Semicolon, 3),
                tok!(MapsTo, 4),
                tok!("=", 5),
                tok!(">", 5),
                tok!(Comma, 6),
            ],
        );
    }

    #[test]
    fn backslash_escape() {
        check_lexer_output("test\\{\\}\\:\\ \\\n", vec![tok!("test{}: \n", 1)])
    }
}
