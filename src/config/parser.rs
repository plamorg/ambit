use crate::config::{ast::*, lexer::*, ParseError, ParseErrorType, ParseResult};

use std::iter::Peekable;

fn expect<I: Iterator<Item = Token>>(
    iter: &mut Peekable<I>,
    choices: &'static [TokType],
) -> ParseResult<TokType> {
    let res = choices
        .iter()
        .find(|ty| iter.peek().map(|x| x.toktype == **ty).unwrap_or(false));
    match res {
        None => Err(ParseError::from(ParseErrorType::Expected(choices))),
        Some(tok) => {
            iter.next();
            Ok(*tok)
        }
    }
}

macro_rules! eat {
    ($it:ident,$t:ident) => {{
        if $it
            .peek()
            .map(|x| x.toktype == TokType::$t)
            .unwrap_or(false)
        {
            $it.next();
            true
        } else {
            false
        }
    }};
    ($it:ident,$s:tt) => {{
        if $it
            .peek()
            .map(|x| x.string.as_ref().map(|s| s == $s).unwrap_or(false))
            .unwrap_or(false)
        {
            $it.next();
            true
        } else {
            false
        }
    }};
}

pub struct Parser<I: Iterator<Item = Token>> {
    iter: Peekable<I>,
}
impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn new(iter: Peekable<I>) -> Self {
        Parser { iter }
    }
}
impl<I: Iterator<Item = Token>> Iterator for Parser<I> {
    type Item = ParseResult<Entry>;
    fn next(&mut self) -> Option<Self::Item> {
        // If there's nothing left, we've consumed all the input - yay!
        match self.iter.peek() {
            None => None,
            Some(_) => Some({
                let new = Entry::parse(&mut self.iter);
                match new {
                    Err(mut e) => {
                        e.tok = self.iter.peek().cloned();
                        Err(e)
                    }
                    Ok(p) => Ok(p),
                }
            }),
        }
    }
}

// entry -> spec ("=>" spec)? ";"
impl Entry {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Entry> {
        let left = Spec::parse(iter)?;
        let mut right = None;
        if eat!(iter, MapsTo) {
            let right_val = Spec::parse(iter)?;
            let left_nr = left.nr_of_options().ok_or(ParseError {
                tok: None,
                ty: ParseErrorType::Custom("Too many options on left hand side"),
            })?;
            let right_nr = right_val.nr_of_options().ok_or(ParseError {
                tok: None,
                ty: ParseErrorType::Custom("Too many options on right hand side"),
            })?;
            if left_nr != right_nr {
                return Err(ParseError::from(ParseErrorType::Custom(
                    "Left and right sides of mapping must match up",
                )));
            }
            right = Some(right_val);
        }
        expect(iter, &[TokType::Semicolon])?;
        Ok(Entry { left, right })
    }
}

/* spec -> str
 *      -> str? variant-expr spec?
 *      -> str? match-expr spec?
 */
impl Spec {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Spec> {
        let mut string = None;
        if iter
            .peek()
            .map(|x| x.toktype == TokType::Str)
            .unwrap_or(false)
        {
            string = Some(
                iter.next()
                    .unwrap()
                    .string
                    .expect("Internal error: string expected!"),
            );
        }
        fn starts_spec<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> bool {
            // Check if a new spec could start here.
            // Note that this should be updated if the spec specification changes.
            iter.peek()
                .map(|next| {
                    [TokType::Str, TokType::LBrace, TokType::LBracket]
                        .iter()
                        .any(|x| next.toktype == *x)
                })
                .unwrap_or(false)
        }
        // optimization
        match iter.peek() {
            None => {}
            Some(tok) => match tok.toktype {
                TokType::LBrace => {
                    return Ok(Spec {
                        string,
                        spectype: SpecType::Match(
                            Box::new(MatchExpr::parse(iter)?),
                            // If we didn't do this hack,
                            // the grammar wouldn't be LL(1).
                            {
                                if starts_spec(iter) {
                                    Some(Box::new(Spec::parse(iter)?))
                                } else {
                                    None
                                }
                            },
                        ),
                    });
                }
                TokType::LBracket => {
                    return Ok(Spec {
                        string,
                        spectype: SpecType::Variant(Box::new(VariantExpr::parse(iter)?), {
                            if starts_spec(iter) {
                                Some(Box::new(Spec::parse(iter)?))
                            } else {
                                None
                            }
                        }),
                    });
                }
                _ => {}
            },
        }
        if string.is_none() {
            Err(ParseError::from(ParseErrorType::Expected(&[TokType::Str])))
        } else {
            Ok(Spec {
                string,
                spectype: SpecType::None,
            })
        }
    }
}

// variant-expr -> [ spec (, spec)* ]
impl VariantExpr {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<VariantExpr> {
        expect(iter, &[TokType::LBracket])?;
        // Better error message.
        if iter
            .peek()
            .map(|x| x.toktype == TokType::RBracket)
            .unwrap_or(false)
        {
            return Err(ParseError::from(ParseErrorType::Custom(
                "Must have at least one option",
            )));
        }
        let mut specs = Vec::new();
        loop {
            specs.push(Spec::parse(iter)?);
            if !eat!(iter, Comma) {
                break;
            }
        }
        expect(iter, &[TokType::RBracket])?;
        Ok(VariantExpr { specs })
    }
}

// match-expr -> { (expr ":" spec ":")* "default" ":" spec }
impl MatchExpr {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<MatchExpr> {
        expect(iter, &[TokType::LBrace])?;
        let mut cases = Vec::new();
        loop {
            if eat!(iter, "default") {
                expect(iter, &[TokType::Colon])?;
                let ret = MatchExpr {
                    cases,
                    default: Spec::parse(iter)?,
                };
                expect(iter, &[TokType::RBrace])?;
                return Ok(ret);
            }
            let expr = Expr::parse(iter)?;
            expect(iter, &[TokType::Colon])?;
            let spec = Spec::parse(iter)?;
            cases.push((expr, spec));
            expect(iter, &[TokType::Comma])?;
        }
    }
}

// expr -> "windows" | "linux" | "macos" | "unix" | "bsd"
// (for now)
impl Expr {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Expr> {
        macro_rules! exprtype {
            ($i:ident) => {{
                iter.next();
                Ok(Expr {
                    exprtype: ExprType::$i,
                })
            }};
        }
        if let Some(tok) = iter.peek() {
            if tok.toktype == TokType::Str {
                if let Some(s) = tok.string.as_ref() {
                    match s.as_str() {
                        "windows" => return exprtype!(Windows),
                        "macos" => return exprtype!(Macos),
                        "linux" => return exprtype!(Linux),
                        "unix" => return exprtype!(Unix),
                        "bsd" => return exprtype!(Bsd),
                        _ => {}
                    }
                }
            }
        }
        Err(ParseError::from(ParseErrorType::Expected(&[TokType::Str])))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // Makes it more convenient to write token lists.
    macro_rules! toklist {
        [$($i:expr),+] => {
            {
                trait StrToToken where Self: ToString {
                    fn repr_as_token(&self) -> Token {
                        Token { string: Some(self.to_string()), line: 0, toktype: TokType::Str }
                    }
                }
                // If the type is a `&str`, make the outputted Token be a TokType::Str.
                impl StrToToken for &str {}
                trait OtherToToken where Self: Into<TokType> + Clone {
                    fn repr_as_token(&self) -> Token {
                        Token { string: None, line: 0, toktype: self.clone().into() }
                    }
                }
                // If the type is a `TokType`, make the outputted Token be that toktype.
                impl OtherToToken for TokType {}
                [$($i.repr_as_token()),+]
            }
        }
    }

    fn success(toks: &[Token], ast: &[Entry]) {
        let iter = toks.iter().cloned().peekable();
        match Parser::new(iter).collect::<ParseResult<Vec<_>>>() {
            Err(e) => panic!("{:?} at token {:?}", e.ty, e.tok),
            Ok(parsed) => assert_eq!(parsed, ast),
        }
    }
    fn fail(toks: &[Token], err: ParseError) {
        let iter = toks.iter().cloned().peekable();
        let res = Parser::new(iter)
            .collect::<ParseResult<Vec<_>>>()
            .unwrap_err();
        assert_eq!(err, res);
    }

    #[test]
    fn basic_entry() {
        success(
            &toklist!["yes", TokType::Semicolon],
            &[Entry {
                left: Spec {
                    string: Some("yes".to_owned()),
                    spectype: SpecType::None,
                },
                right: None,
            }],
        );
    }

    #[test]
    fn choice_expr_basic() {
        success(
            &toklist![
                TokType::LBracket,
                "a",
                TokType::Comma,
                "b",
                TokType::RBracket,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec {
                    string: None,
                    spectype: SpecType::Variant(
                        Box::new(VariantExpr {
                            specs: vec![
                                Spec {
                                    string: Some("a".to_owned()),
                                    spectype: SpecType::None,
                                },
                                Spec {
                                    string: Some("b".to_owned()),
                                    spectype: SpecType::None,
                                },
                            ],
                        }),
                        None,
                    ),
                },
                right: None,
            }],
        );
    }

    #[test]
    fn comp_expr_basic() {
        success(
            &toklist![
                TokType::LBrace,
                "windows",
                TokType::Colon,
                "a",
                TokType::Comma,
                "default",
                TokType::Colon,
                "b",
                TokType::RBrace,
                "c",
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec {
                    string: None,
                    spectype: SpecType::Match(
                        Box::new(MatchExpr {
                            cases: vec![(
                                Expr {
                                    exprtype: ExprType::Windows,
                                },
                                Spec {
                                    string: Some("a".to_owned()),
                                    spectype: SpecType::None,
                                },
                            )],
                            default: Spec {
                                string: Some("b".to_owned()),
                                spectype: SpecType::None,
                            },
                        }),
                        Some(Box::new(Spec {
                            string: Some("c".to_owned()),
                            spectype: SpecType::None,
                        })),
                    ),
                },
                right: None,
            }],
        );
    }

    #[test]
    fn multiple_exprs() {
        success(
            &toklist![
                "examples of ",
                TokType::LBracket,
                "gui",
                TokType::Comma,
                "cli",
                TokType::RBracket,
                TokType::MapsTo,
                TokType::LBracket,
                "gvim",
                TokType::Comma,
                "ed",
                TokType::RBracket,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec {
                    string: Some("examples of ".to_owned()),
                    spectype: SpecType::Variant(
                        Box::new(VariantExpr {
                            specs: vec![
                                (Spec {
                                    string: Some("gui".to_owned()),
                                    spectype: SpecType::None,
                                }),
                                (Spec {
                                    string: Some("cli".to_owned()),
                                    spectype: SpecType::None,
                                }),
                            ],
                        }),
                        None,
                    ),
                },
                right: Some(Spec {
                    string: None,
                    spectype: SpecType::Variant(
                        Box::new(VariantExpr {
                            specs: vec![
                                (Spec {
                                    string: Some("gvim".to_owned()),
                                    spectype: SpecType::None,
                                }),
                                (Spec {
                                    string: Some("ed".to_owned()),
                                    spectype: SpecType::None,
                                }),
                            ],
                        }),
                        None,
                    ),
                }),
            }],
        );
    }

    #[test]
    fn nested_variant_with_only_one_option() {
        success(
            &toklist![
                ".config/",
                TokType::LBracket,
                "kitty/",
                TokType::LBracket,
                "kitty.conf",
                TokType::Comma,
                "theme.conf",
                TokType::RBracket,
                TokType::RBracket,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec {
                    string: Some(".config/".to_owned()),
                    spectype: SpecType::Variant(
                        Box::new(VariantExpr {
                            specs: vec![Spec {
                                string: Some("kitty/".to_owned()),
                                spectype: SpecType::Variant(
                                    Box::new(VariantExpr {
                                        specs: vec![
                                            Spec {
                                                string: Some("kitty.conf".to_owned()),
                                                spectype: SpecType::None,
                                            },
                                            Spec {
                                                string: Some("theme.conf".to_owned()),
                                                spectype: SpecType::None,
                                            },
                                        ],
                                    }),
                                    None,
                                ),
                            }],
                        }),
                        None,
                    ),
                },
                right: None,
            }],
        );
    }

    #[test]
    fn semicolon_error() {
        fail(
            &toklist!["a"],
            ParseError {
                tok: None, // `None` means it failed at EOF
                ty: ParseErrorType::Expected(&[TokType::Semicolon]),
            },
        );
    }
    // TODO: add more tests
}
