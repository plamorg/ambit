use crate::lexer::*;

use std::iter::Peekable;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ParseError {
    Expected(&'static [TokType]),
    Custom(&'static str),
}

pub type Result<T> = std::result::Result<T, ParseError>;

macro_rules! expect {
    ($it:ident,$l:tt) => {{
        let res = $l
            .iter()
            .find(|ty| $it.peek().map(|x| x.toktype == **ty).unwrap_or(false));
        match res {
            None => return Err(ParseError::Expected(&$l)),
            Some(tok) => {
                $it.next();
                tok
            }
        }
    }};
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

macro_rules! ends {
    ($it:ident) => {
        $it.peek().is_none()
    };
}

pub struct Parser<'a, I: Iterator<Item = Token>> {
    iter: &'a mut Peekable<I>,
}
impl<'a, I: Iterator<Item = Token>> Parser<'a, I> {
    #[allow(dead_code)]
    fn new(iter: &'a mut Peekable<I>) -> Self {
        Parser { iter }
    }
}
impl<'a, I: Iterator<Item = Token>> Iterator for Parser<'a, I> {
    type Item = Result<Entry>;
    fn next(&mut self) -> Option<Self::Item> {
        // If there's nothing left, we've consumed all the input - yay!
        match self.iter.peek() {
            None => None,
            Some(_) => Some(Entry::parse(self.iter)),
        }
    }
}

// entry -> spec ("=>" spec)? ";"
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Entry {
    left: Spec,
    right: Option<Spec>,
}
impl Entry {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<Entry> {
        let left = Spec::parse(iter)?;
        let mut right = None;
        if eat!(iter, MapsTo) {
            let right_val = Spec::parse(iter)?;
            let left_nr = left
                .nr_of_options()
                .ok_or(ParseError::Custom("Too many options on left hand side"))?;
            let right_nr = right_val
                .nr_of_options()
                .ok_or(ParseError::Custom("Too many options on right hand side"))?;
            if left_nr != right_nr {
                return Err(ParseError::Custom(
                    "Left and right sides of mapping must match up",
                ));
            }
            right = Some(right_val);
        }
        expect!(iter, [TokType::Semicolon]);
        Ok(Entry { left, right })
    }
}

/* spec -> str
 *      -> str? choice-expr spec?
 *      -> str? comp-expr spec?
 */
#[derive(PartialEq, Eq, Debug, Clone)]
struct Spec {
    string: Option<String>,
    spectype: SpecType,
}
#[derive(PartialEq, Eq, Debug, Clone)]
enum SpecType {
    None,
    Choice(Box<ChoiceExpr>, Option<Box<Spec>>),
    Comp(Box<CompExpr>, Option<Box<Spec>>),
}
impl Spec {
    /* Returns None if the nr. of options
     * overflows `usize`.
     */
    pub fn nr_of_options(&self) -> Option<usize> {
        match &self.spectype {
            SpecType::None => Some(1),
            SpecType::Comp(_, spec) => spec.as_ref().map(|s| s.nr_of_options()).unwrap_or(Some(1)),
            SpecType::Choice(expr, spec) => {
                let exprnr = expr.nr_of_options()?;
                let specnr = spec
                    .as_ref()
                    .map(|s| s.nr_of_options())
                    .unwrap_or(Some(1))?;
                exprnr.checked_mul(specnr)
            }
        }
    }
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<Spec> {
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
        fn probably_ends_spec<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> bool {
            // Check if the iterator "probably ends" here,
            // or we need to do another round of parsing.
            ends!(iter)
                || iter
                    .peek()
                    .map(|next| {
                        [TokType::Semicolon, TokType::MapsTo]
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
                        spectype: SpecType::Comp(
                            Box::new(CompExpr::parse(iter)?),
                            // If we didn't do this hack,
                            // the grammar wouldn't be LL(1).
                            {
                                if probably_ends_spec(iter) {
                                    // It ends here.
                                    None
                                } else {
                                    // It *probably* doesn't end here.
                                    Some(Box::new(Spec::parse(iter)?))
                                }
                            },
                        ),
                    });
                }
                TokType::LBracket => {
                    return Ok(Spec {
                        string,
                        spectype: SpecType::Choice(Box::new(ChoiceExpr::parse(iter)?), {
                            if probably_ends_spec(iter) {
                                // It ends here.
                                None
                            } else {
                                // It *probably* doesn't end here.
                                Some(Box::new(Spec::parse(iter)?))
                            }
                        }),
                    });
                }
                _ => {}
            },
        }
        if string.is_none() {
            Err(ParseError::Expected(&[TokType::Str]))
        } else {
            Ok(Spec {
                string,
                spectype: SpecType::None,
            })
        }
    }
}

// choice-expr -> [ spec (, spec)* ]
#[derive(PartialEq, Eq, Debug, Clone)]
struct ChoiceExpr {
    specs: Vec<Spec>,
}
impl ChoiceExpr {
    // Returns None if the nr of options is larger than usize::MAX.
    pub fn nr_of_options(&self) -> Option<usize> {
        self.specs.iter().try_fold(0usize, |nr, spec| {
            spec.nr_of_options()
                .and_then(|specnr| specnr.checked_add(nr))
        })
    }
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<ChoiceExpr> {
        expect!(iter, [TokType::LBracket]);
        // Better error message.
        if iter
            .peek()
            .map(|x| x.toktype == TokType::RBracket)
            .unwrap_or(false)
        {
            return Err(ParseError::Custom("Must have at least one option"));
        }
        let mut specs = Vec::new();
        loop {
            specs.push(Spec::parse(iter)?);
            if !eat!(iter, Comma) {
                break;
            }
        }
        expect!(iter, [TokType::RBracket]);
        Ok(ChoiceExpr { specs })
    }
}

// choice-expr -> { (expr ":" spec ":")* "default" ":" spec }
#[derive(PartialEq, Eq, Debug, Clone)]
struct CompExpr {
    cases: Vec<(Expr, Spec)>,
    default: Spec,
}
impl CompExpr {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<CompExpr> {
        expect!(iter, [TokType::LBrace]);
        let mut cases = Vec::new();
        loop {
            if eat!(iter, "default") {
                expect!(iter, [TokType::Colon]);
                let ret = CompExpr {
                    cases,
                    default: Spec::parse(iter)?,
                };
                expect!(iter, [TokType::RBrace]);
                return Ok(ret);
            }
            let expr = Expr::parse(iter)?;
            expect!(iter, [TokType::Colon]);
            let spec = Spec::parse(iter)?;
            cases.push((expr, spec));
            expect!(iter, [TokType::Comma]);
        }
    }
}

// expr -> "windows" | "linux" | "macos" | "unix" | "bsd"
// (for now)
#[derive(PartialEq, Eq, Debug, Clone)]
struct Expr {
    exprtype: ExprType,
}
#[derive(PartialEq, Eq, Debug, Clone)]
enum ExprType {
    Windows,
    Linux,
    Macos,
    Unix,
    Bsd,
}
impl Expr {
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<Expr> {
        macro_rules! exprtype {
            ($i:ident) => {{
                iter.next();
                Ok(Expr {
                    exprtype: ExprType::$i,
                })
            }};
        }
        match iter.peek() {
            Some(tok) if tok.toktype == TokType::Str => match tok.string.as_ref() {
                None => {}
                Some(s) => match s.as_str() {
                    "windows" => return exprtype!(Windows),
                    "macos" => return exprtype!(Macos),
                    "linux" => return exprtype!(Linux),
                    "unix" => return exprtype!(Unix),
                    "bsd" => return exprtype!(Bsd),
                    _ => {}
                },
            },
            _ => {}
        }
        Err(ParseError::Expected(&[TokType::Str]))
    }
}

#[test]
fn test_parser() {
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
        let mut iter = toks.iter().cloned().peekable();
        match Parser::new(&mut iter).collect::<Result<Vec<_>>>() {
            Err(e) => panic!("{:?} at token {:?}", e, iter.peek()),
            Ok(parsed) => assert_eq!(parsed, ast),
        }
    }
    fn fail(toks: &[Token], err: ParseError) {
        let mut iter = toks.iter().cloned().peekable();
        let res = Parser::new(&mut iter)
            .collect::<Result<Vec<_>>>()
            .unwrap_err();
        assert_eq!(err, res);
    }

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
                spectype: SpecType::Choice(
                    Box::new(ChoiceExpr {
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
                spectype: SpecType::Comp(
                    Box::new(CompExpr {
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
                spectype: SpecType::Choice(
                    Box::new(ChoiceExpr {
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
                spectype: SpecType::Choice(
                    Box::new(ChoiceExpr {
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

    fail(&toklist!["a"], ParseError::Expected(&[TokType::Semicolon]));
    // TODO: add more tests
}
