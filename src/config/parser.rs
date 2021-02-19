use crate::config::{ast::*, lexer::*, ParseError, ParseErrorType, ParseResult};

use std::iter::Peekable;

// Can be simply parsed.
pub trait SimpleParse
where
    Self: Sized,
{
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self>;
}

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
fn eat<I: Iterator<Item = Token>>(iter: &mut Peekable<I>, ty: TokType) -> bool {
    if iter.peek().map(|x| x.toktype == ty).unwrap_or(false) {
        iter.next();
        true
    } else {
        false
    }
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
impl SimpleParse for Entry {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
        let left = Spec::parse(iter)?;
        let mut right = None;
        if eat(iter, TokType::MapsTo) {
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
impl SimpleParse for Spec {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
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
        fn try_parse_spec<I: Iterator<Item = Token>>(
            iter: &mut Peekable<I>,
        ) -> ParseResult<Option<Box<Spec>>> {
            // Check if a new spec could start here.
            // Note that this should be updated if the spec specification changes.
            fn is_starting_token(next: &Token) -> bool {
                [TokType::Str, TokType::LBrace, TokType::LBracket]
                    .iter()
                    .any(|x| next.toktype == *x)
            }
            if iter.peek().map(is_starting_token).unwrap_or(false) {
                Ok(Some(Box::new(Spec::parse(iter)?)))
            } else {
                Ok(None)
            }
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
                            try_parse_spec(iter)?,
                        ),
                    });
                }
                TokType::LBracket => {
                    return Ok(Spec {
                        string,
                        spectype: SpecType::Variant(
                            Box::new(VariantExpr::parse(iter)?),
                            try_parse_spec(iter)?,
                        ),
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
impl SimpleParse for VariantExpr {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
        expect(iter, &[TokType::LBracket])?;
        // Better error message.
        if iter
            .peek()
            .map(|x| x.toktype == TokType::RBracket)
            .unwrap_or(false)
        {
            return Err(ParseError::from(ParseErrorType::Custom(
                "Variant expression have at least one option",
            )));
        }
        Ok(VariantExpr {
            specs: CommaList::parse(iter, TokType::RBracket)?.list,
        })
    }
}

// match-expr -> { comma-list<(expr ":" spec)> }
impl SimpleParse for MatchExpr {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
        // Allow `expr ":" spec` to be parsed into a tuple `(expr, spec)`.
        impl SimpleParse for (Expr, Spec) {
            fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
                let expr = Expr::parse(iter)?;
                expect(iter, &[TokType::Colon])?;
                let spec = Spec::parse(iter)?;
                Ok((expr, spec))
            }
        }
        expect(iter, &[TokType::LBrace])?;
        Ok(MatchExpr {
            cases: CommaList::parse(iter, TokType::RBrace)?.list,
        })
    }
}

// comma-list<T> -> (T ",")* T?
impl<T: SimpleParse> CommaList<T> {
    pub fn parse<I: Iterator<Item = Token>>(
        iter: &mut Peekable<I>,
        end: TokType,
    ) -> ParseResult<Self> {
        let mut list = Vec::new();
        loop {
            // Allow trailing comma
            if eat(iter, end) {
                break;
            }
            list.push(T::parse(iter)?);
            if eat(iter, end) {
                break;
            }
            expect(iter, &[TokType::Comma])?;
        }
        Ok(Self { list })
    }
}

// expr -> "windows" | "linux" | "macos" | "unix" | "bsd"
// (for now)
impl SimpleParse for Expr {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
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
                        "default" => return exprtype!(Any),
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
                left: Spec::from("yes"),
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
                    spectype: SpecType::variant_expr(vec![Spec::from("a"), Spec::from("b")], None),
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
                    spectype: SpecType::match_expr(
                        vec![
                            (ExprType::Windows.into(), Spec::from("a")),
                            (ExprType::Any.into(), Spec::from("b")),
                        ],
                        Some(Spec::from("c")),
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
                    spectype: SpecType::variant_expr(
                        vec![Spec::from("gui"), Spec::from("cli")],
                        None,
                    ),
                },
                right: Some(Spec {
                    string: None,
                    spectype: SpecType::variant_expr(
                        vec![Spec::from("gvim"), Spec::from("ed")],
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
                    spectype: SpecType::variant_expr(
                        vec![Spec {
                            string: Some("kitty/".to_owned()),
                            spectype: SpecType::variant_expr(
                                vec![Spec::from("kitty.conf"), Spec::from("theme.conf")],
                                None,
                            ),
                        }],
                        None,
                    ),
                },
                right: None,
            }],
        );
    }

    #[test]
    fn match_expr_without_default() {
        success(
            &toklist![
                TokType::LBrace,
                "linux",
                TokType::Colon,
                "a",
                TokType::Comma,
                "macos",
                TokType::Colon,
                "b",
                TokType::RBrace,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec {
                    string: None,
                    spectype: SpecType::match_expr(
                        vec![
                            (ExprType::Linux.into(), Spec::from("a")),
                            (ExprType::Macos.into(), Spec::from("b")),
                        ],
                        None,
                    ),
                },
                right: None,
            }],
        )
    }

    #[test]
    fn variant_trailing_comma() {
        success(
            &toklist![
                TokType::LBracket,
                "a",
                TokType::Comma,
                TokType::RBracket,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec {
                    string: None,
                    spectype: SpecType::variant_expr(vec![Spec::from("a")], None),
                },
                right: None,
            }],
        )
    }

    #[test]
    fn match_trailing_comma() {
        success(
            &toklist![
                TokType::LBrace,
                "linux",
                TokType::Colon,
                "a",
                TokType::Comma,
                TokType::RBrace,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec {
                    string: None,
                    spectype: SpecType::match_expr(
                        vec![(ExprType::Linux.into(), Spec::from("a"))],
                        None,
                    ),
                },
                right: None,
            }],
        )
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
