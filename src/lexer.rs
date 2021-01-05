use std::iter::Peekable;

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum TokType {
    Str(String),
    RBrace,
    LBrace,
    RBracket,
    LBracket,
    MapsTo,
    Comma,
    Semicolon
}

impl TokType {
    pub fn str(s: &str) -> TokType {
        TokType::Str(s.to_string())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Token {
    pub toktype: TokType,
    pub line: usize
}

impl Token {
    pub fn new(toktype: TokType, line: usize) -> Token {
        Token {
            toktype,
            line
        }
    }
    pub fn str(s: &str, line: usize) -> Token {
        Token {
            toktype: TokType::str(s),
            line
        }
    }
}

pub struct Lexer<'a, I: Iterator<Item=char>> {
    iter: &'a mut Peekable<I>,
    line: usize
}

impl<'a, I: Iterator<Item=char>> Lexer<'a, I> {
    pub fn new(iter: &'a mut Peekable<I>) -> Lexer<'a, I> {
        Lexer {
            iter,
            line: 1
        }
    }
}

impl<'a, I: Iterator<Item=char>> Iterator for Lexer<'a, I> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        fn proc_str<I: Iterator<Item=char>>(iter: &mut Peekable<I>, start: char) -> TokType {
            let mut ret = start.to_string();
            while iter.peek().map(|c| !c.is_ascii_whitespace()).unwrap_or(false) {
                ret.push(iter.next().unwrap());
            }
            TokType::Str(ret)
        }

        fn next_tok_type<I: Iterator<Item=char>>(iter: &mut Peekable<I>, line: &mut usize) -> Option<TokType> {
            loop {
                match iter.next() {
                    None => return None,
                    Some(chr) => match chr {
                        '\n' => *line += 1,
                        '{' => return Some(TokType::LBrace),
                        '}' => return Some(TokType::RBrace),
                        '[' => return Some(TokType::LBracket),
                        ']' => return Some(TokType::RBracket),
                        ',' => return Some(TokType::Comma),
                        ';' => return Some(TokType::Semicolon),
                        '=' => {
                            if iter.peek().map(|x| *x == '>').unwrap_or(false) {
                                iter.next();
                                return Some(TokType::MapsTo);
                            } else {
                                return Some(proc_str(iter, '='));
                            }
                        },
                        ' ' | '\t' | '\r' => {},
                        _ => return Some(proc_str(iter, chr))
                    }
                }
            }
        }

        next_tok_type(self.iter, &mut self.line)
            .map(|toktype| Token { toktype, line: self.line })
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

    check("~/.config/nvim/init.vim => init.vim", vec![
        Token::new(TokType::str("~/.config/nvim/init.vim"), 1),
        Token::new(TokType::MapsTo, 1),
        Token::new(TokType::str("init.vim"), 1)
    ]);

    check("ok then \n \t\t\t\t\tpls", vec![
        Token::str("ok", 1), 
        Token::str("then", 1), 
        Token::str("pls", 2)
    ]);

    check("{ }\n [ ]\n ; \n =>\t\n = >\n ,\n", vec![
        Token::new(TokType::LBrace, 1),
        Token::new(TokType::RBrace, 1),
        Token::new(TokType::LBracket, 2),
        Token::new(TokType::RBracket, 2),
        Token::new(TokType::Semicolon, 3),
        Token::new(TokType::MapsTo, 4),
        Token::str("=", 5), Token::str(">", 5),
        Token::new(TokType::Comma, 6)
    ]);

}
