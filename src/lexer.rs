use std::iter::Peekable;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum TokType {
    Str,
    RBrace,
    LBrace,
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
                .map(|c| !c.is_ascii_whitespace())
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

#[test]
fn test_lexer() {
    fn check(s: &str, t: Vec<Token>) {
        let mut chars = s.chars().peekable();
        let lex = Lexer::new(&mut chars);
        let res = lex.collect::<Vec<Token>>();
        if res != t {
            panic!("Failed test with {}:\n{:?}\n!=\n{:?}", s, res, t);
        }
    }

    macro_rules! tok {
        ($t:ident, $l:literal) => {
            Token::new(TokType::$t, $l)
        };
        ($s:tt, $l:literal) => {
            Token::string($s.to_string(), $l)
        };
    }

    check(
        "~/.config/nvim/init.vim => init.vim",
        vec![
            tok!("~/.config/nvim/init.vim", 1),
            tok!(MapsTo, 1),
            tok!("init.vim", 1),
        ],
    );

    check(
        "ok then \n \t\t\t\t\tpls",
        vec![tok!("ok", 1), tok!("then", 1), tok!("pls", 2)],
    );

    check(
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
