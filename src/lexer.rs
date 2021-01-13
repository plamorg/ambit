use std::iter::Peekable;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum TokType {
    Str,
    // "Brace" refers to curly braces: { and }.
    RBrace,
    LBrace,
    // "Bracket" refers to square brackets: [ and ].
    RBracket,
    LBracket,
    MapsTo,
    Comma,
    Colon,
    Semicolon,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Token {
    pub toktype: TokType,
    pub string: Option<String>,
    pub line: usize,
}

impl Token {
    pub fn new(toktype: TokType, line: usize) -> Token {
        Token {
            toktype,
            string: None,
            line,
        }
    }
    pub fn string(s: String, line: usize) -> Token {
        Token {
            toktype: TokType::Str,
            string: Some(s),
            line,
        }
    }
}

pub struct Lexer<'a, I: Iterator<Item = char>> {
    iter: &'a mut Peekable<I>,
    line: usize,
}

impl<'a, I: Iterator<Item = char>> Lexer<'a, I> {
    #[allow(dead_code)]
    pub fn new(iter: &'a mut Peekable<I>) -> Lexer<'a, I> {
        Lexer { iter, line: 1 }
    }
}

impl<'a, I: Iterator<Item = char>> Iterator for Lexer<'a, I> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        fn proc_str<I: Iterator<Item = char>>(iter: &mut Peekable<I>, start: char) -> String {
            let mut ret = start.to_string();
            while iter
                .peek()
                .map(|c| {
                    !c.is_ascii_whitespace()
                        && !['{', '}', '[', ']', ',', ';', ':', '=']
                            .iter()
                            .any(|e| e == c)
                })
                .unwrap_or(false)
            {
                ret.push(iter.next().unwrap());
            }
            ret
        }

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
                            return Some(Token::string(proc_str(self.iter, '='), self.line));
                        }
                    }
                    ' ' | '\t' | '\r' => {}
                    _ => return Some(Token::string(proc_str(self.iter, chr), self.line)),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_lexer_output(input: &str, expected: Vec<Token>) {
        let mut chars = input.chars().peekable();
        let lex = Lexer::new(&mut chars);
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
            Token::string($s.to_string(), $l)
        };
    }

    #[test]
    fn full_example_config() {
        check_lexer_output(
            "\
~/.config/nvim/init.vim => config.nvim;
~/{
    windows: _config,
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
                tok!("windows", 3),
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
            "{ }\n [ ]\n ; \n =>\t\n = >\n ,\n",
            vec![
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
}
