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
        Some(_) => Ok(iter.next().unwrap().toktype),
    }
}

/* Returns if the next element from the iterator `iter` has toktype `ty`,
 * without advancing the iterator.
 */
fn next_is<I: Iterator<Item = Token>>(iter: &mut Peekable<I>, ty: &TokType) -> bool {
    iter.peek().map(|x| x.toktype == *ty).unwrap_or(false)
}

fn eat<I: Iterator<Item = Token>>(iter: &mut Peekable<I>, ty: &TokType) -> bool {
    if next_is(iter, ty) {
        iter.next();
        true
    } else {
        false
    }
}

// Helpful SimpleParse type.
impl SimpleParse for String {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
        if let Some(Token {
            toktype: TokType::Str(_),
            ..
        }) = iter.peek()
        {
            if let Some(Token {
                toktype: TokType::Str(s),
                ..
            }) = iter.next()
            {
                return Ok(s);
            }
        }
        Err(ParseError::from(ParseErrorType::Expected(EXPECTED_STR)))
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
                        e.tok = self.iter.next();
                        while Entry::parse(&mut self.iter).is_err() {
                            // If an error has been encountered, continue iterating until a non-error entry is found.
                            // Contiguous errors are a by-product of the initial error and shouldn't be reported.
                            if self.iter.next().is_none() {
                                break;
                            }
                        }
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
        if eat(iter, &TokType::MapsTo) {
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
        if let Some(Token {
            toktype: TokType::Str(_),
            ..
        }) = iter.peek()
        {
            string = Some(iter.next().unwrap().toktype.unwrap_str());
        }
        fn try_parse_spec<I: Iterator<Item = Token>>(
            iter: &mut Peekable<I>,
        ) -> ParseResult<Option<Box<Spec>>> {
            // Check if a new spec could start here.
            // Note that this should be updated if the spec specification changes.
            fn is_starting_token(next: &Token) -> bool {
                matches!(
                    next.toktype,
                    TokType::Str(_) | TokType::LBrace | TokType::LBracket
                )
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
            Err(ParseError::from(ParseErrorType::Expected(EXPECTED_STR)))
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
        if next_is(iter, &TokType::RBracket) {
            return Err(ParseError::from(ParseErrorType::Custom(
                "Variant expression must have at least one option",
            )));
        }
        Ok(VariantExpr {
            specs: CommaList::parse(iter, &TokType::RBracket)?.list,
        })
    }
}

// match-expr -> { comma-list<(expr ":" spec)> }
impl SimpleParse for MatchExpr {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
        expect(iter, &[TokType::LBrace])?;
        // Allow `expr ":" spec` to be parsed into a tuple `(expr, spec)`.
        // (This would be confusing if placed in outer scope,
        // since it's unnecessary, so it's placed here.)
        impl SimpleParse for (Expr, Spec) {
            fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
                let expr = Expr::parse(iter)?;
                expect(iter, &[TokType::Colon])?;
                let spec = Spec::parse(iter)?;
                Ok((expr, spec))
            }
        }
        Ok(MatchExpr {
            cases: CommaList::parse(iter, &TokType::RBrace)?.list,
        })
    }
}

// comma-list<T> -> (T ",")* T?
// Note that CommaList does not implement SimpleParse.
impl<T: SimpleParse> CommaList<T> {
    pub fn parse<I: Iterator<Item = Token>>(
        iter: &mut Peekable<I>,
        // What token the comma-list should end at, such as RBrace or RBracket.
        // (Required because computers aren't good enough at parsing :/)
        end: &TokType,
    ) -> ParseResult<Self> {
        let mut list = Vec::new();
        while !eat(iter, end) {
            list.push(T::parse(iter)?);
            // Allow list without trailing comma
            if eat(iter, end) {
                break;
            }
            expect(iter, &[TokType::Comma])?;
        }
        Ok(Self { list })
    }
}

// expr -> ( "os" | "host" ) "(" comma-list<str> ")"
//       | "default"
impl SimpleParse for Expr {
    fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> ParseResult<Self> {
        let err = ParseError::from(ParseErrorType::Expected(EXPECTED_STR));
        let expr_type: fn(Vec<String>) -> Expr;
        match iter.peek() {
            Some(Token {
                toktype: TokType::Str(s),
                ..
            }) => match s.as_str() {
                "os" => expr_type = Expr::Os,
                "host" => expr_type = Expr::Host,
                "!os" => expr_type = Expr::NotOs,
                "!host" => expr_type = Expr::NotHost,
                "default" => {
                    // "default" takes no strings to check (since it's always true).
                    iter.next();
                    return Ok(Expr::Any);
                }
                _ => return Err(err),
            },
            _ => return Err(err),
        }
        iter.next();
        expect(iter, &[TokType::LParen])?;
        Ok(expr_type(CommaList::parse(iter, &TokType::RParen)?.list))
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
                        Token { line: 0, toktype: TokType::Str(self.to_string()) }
                    }
                }
                // If the type is a `&str`, make the outputted Token be a TokType::Str.
                impl StrToToken for &str {}
                trait OtherToToken where Self: Into<TokType> + Clone {
                    fn repr_as_token(&self) -> Token {
                        Token { line: 0, toktype: self.clone().into() }
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
    fn fail(toks: &[Token], errors: Vec<ParseError>) {
        // Take a vector of errors to check for multiple errors if needed.
        let iter = toks.iter().cloned().peekable();
        let res: Vec<_> = Parser::new(iter).filter_map(|e| e.err()).collect();
        assert_eq!(errors, res);
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
                left: Spec::from(SpecType::variant_expr(
                    vec![Spec::from("a"), Spec::from("b")],
                    None,
                )),
                right: None,
            }],
        );
    }

    #[test]
    fn match_expr_basic() {
        success(
            &toklist![
                TokType::LBrace,
                "default",
                TokType::Colon,
                "b",
                TokType::Comma,
                "os",
                TokType::LParen,
                "windows",
                TokType::RParen,
                TokType::Colon,
                "a",
                TokType::RBrace,
                "c",
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec::from(SpecType::match_expr(
                    vec![
                        (Expr::Any, Spec::from("b")),
                        (Expr::Os(vec!["windows".to_owned()]), Spec::from("a")),
                    ],
                    Some(Spec::from("c")),
                )),
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
                right: Some(Spec::from(SpecType::variant_expr(
                    vec![Spec::from("gvim"), Spec::from("ed")],
                    None,
                ))),
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
                "host",
                TokType::LParen,
                "hexagon",
                TokType::RParen,
                TokType::Colon,
                "a",
                TokType::Comma,
                "os",
                TokType::LParen,
                "macos",
                TokType::RParen,
                TokType::Colon,
                "b",
                TokType::RBrace,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec::from(SpecType::match_expr(
                    vec![
                        (Expr::Host(vec!["hexagon".to_owned()]), Spec::from("a")),
                        (Expr::Os(vec!["macos".to_owned()]), Spec::from("b")),
                    ],
                    None,
                )),
                right: None,
            }],
        )
    }

    // Trailing commas are valid syntax and must be supported.

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
                left: Spec::from(SpecType::variant_expr(vec![Spec::from("a")], None)),
                right: None,
            }],
        )
    }

    #[test]
    fn match_trailing_comma() {
        success(
            &toklist![
                TokType::LBrace,
                "os",
                TokType::LParen,
                "linux",
                TokType::Comma,
                "windows",
                TokType::Comma, // also checks trailing comma on `os(linux, windows, )`
                TokType::RParen,
                TokType::Colon,
                "a",
                TokType::Comma,
                TokType::RBrace,
                TokType::Semicolon
            ],
            &[Entry {
                left: Spec::from(SpecType::match_expr(
                    vec![(
                        Expr::Os(vec!["linux".to_owned(), "windows".to_owned()]),
                        Spec::from("a"),
                    )],
                    None,
                )),
                right: None,
            }],
        )
    }

    #[test]
    fn semicolon_error() {
        fail(
            &toklist!["a"],
            vec![ParseError {
                tok: None, // `None` means it failed at EOF
                ty: ParseErrorType::Expected(&[TokType::Semicolon]),
            }],
        );
    }

    #[test]
    fn multiple_errors() {
        // Here we are testing that multiple errors can be reported.
        // Only one error should be reported per invalid entry.
        // This should be done while still being able to parse valid entries.
        let toks = &toklist![
            // This first entry should be invalid.
            ".config/bspwm/",
            TokType::LBrace,
            "os",
            TokType::LParen,
            "linux",
            TokType::RParen,
            // Missing colon...
            TokType::LBrace,
            "host",
            TokType::LParen,
            "claby2",
            TokType::Colon,
            "a",
            TokType::Comma,
            "default",
            TokType::Colon,
            "b",
            TokType::Comma,
            TokType::RBrace,
            TokType::RBrace,
            TokType::MapsTo,
            "c",
            TokType::Semicolon,
            // The following entry should be valid.
            "yes",
            TokType::Semicolon,
            // The following entry should be invalid.
            "file" // Missing semicolon...
        ];
        let iter = toks.iter().cloned().peekable();
        let (entries, errors): (Vec<_>, Vec<_>) = Parser::new(iter).partition(Result::is_ok);
        let entries: Vec<_> = entries.into_iter().map(Result::unwrap).collect();
        let errors: Vec<_> = errors.into_iter().map(Result::unwrap_err).collect();
        // Check if the 'yes' entry passed to ensure that it isn't consumed accidentally.
        assert_eq!(
            entries,
            vec![Entry {
                left: Spec::from("yes"),
                right: None,
            },]
        );
        assert_eq!(
            errors,
            vec![
                ParseError {
                    tok: Some(Token::new(TokType::LBrace, 0)),
                    ty: ParseErrorType::Expected(&[TokType::Colon]),
                },
                ParseError {
                    tok: None,
                    ty: ParseErrorType::Expected(&[TokType::Semicolon]),
                }
            ]
        );
    }

    // TODO: add more tests
}
