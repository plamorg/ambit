use crate::config::{lexer::*, ParseError, ParseResult};

use std::iter::Peekable;

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
                    Err(e) => Err((self.iter.peek().cloned(), e)),
                    Ok(p) => Ok(p),
                }
            }),
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
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<Entry, ParseError> {
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

// A `Spec` specifies a fragment of a path, e.g. "~/.config/[nvim/init.vim, spectrwm.conf]".
/* spec -> str
 *      -> str? variant-expr spec?
 *      -> str? match-expr spec?
 */
#[derive(PartialEq, Eq, Debug, Clone)]
struct Spec {
    string: Option<String>,
    spectype: SpecType,
}
#[derive(PartialEq, Eq, Debug, Clone)]
enum SpecType {
    None,
    Variant(Box<VariantExpr>, Option<Box<Spec>>),
    Match(Box<MatchExpr>, Option<Box<Spec>>),
}
impl Spec {
    /* Returns None if the nr. of options
     * overflows `usize`.
     */
    pub fn nr_of_options(&self) -> Option<usize> {
        match &self.spectype {
            SpecType::None => Some(1),
            SpecType::Match(_, spec) => spec.as_ref().map(|s| s.nr_of_options()).unwrap_or(Some(1)),
            SpecType::Variant(expr, spec) => {
                let exprnr = expr.nr_of_options()?;
                let specnr = spec
                    .as_ref()
                    .map(|s| s.nr_of_options())
                    .unwrap_or(Some(1))?;
                exprnr.checked_mul(specnr)
            }
        }
    }
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<Spec, ParseError> {
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
                        spectype: SpecType::Match(
                            Box::new(MatchExpr::parse(iter)?),
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
                        spectype: SpecType::Variant(Box::new(VariantExpr::parse(iter)?), {
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

// variant-expr -> [ spec (, spec)* ]
#[derive(PartialEq, Eq, Debug, Clone)]
struct VariantExpr {
    specs: Vec<Spec>,
}
impl VariantExpr {
    // Returns None if the nr of options is larger than usize::MAX.
    pub fn nr_of_options(&self) -> Option<usize> {
        self.specs.iter().try_fold(0usize, |nr, spec| {
            spec.nr_of_options()
                .and_then(|specnr| specnr.checked_add(nr))
        })
    }
    pub fn parse<I: Iterator<Item = Token>>(
        iter: &mut Peekable<I>,
    ) -> Result<VariantExpr, ParseError> {
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
        Ok(VariantExpr { specs })
    }
}

// Matches, based on the expr, which spec to produce.
// match-expr -> { (expr ":" spec ":")* "default" ":" spec }
#[derive(PartialEq, Eq, Debug, Clone)]
struct MatchExpr {
    cases: Vec<(Expr, Spec)>,
    default: Spec,
}
impl MatchExpr {
    pub fn parse<I: Iterator<Item = Token>>(
        iter: &mut Peekable<I>,
    ) -> Result<MatchExpr, ParseError> {
        expect!(iter, [TokType::LBrace]);
        let mut cases = Vec::new();
        loop {
            if eat!(iter, "default") {
                expect!(iter, [TokType::Colon]);
                let ret = MatchExpr {
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
    pub fn parse<I: Iterator<Item = Token>>(iter: &mut Peekable<I>) -> Result<Expr, ParseError> {
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
        Err(ParseError::Expected(&[TokType::Str]))
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
            Err(e) => panic!("{:?} at token {:?}", e.1, e.0),
            Ok(parsed) => assert_eq!(parsed, ast),
        }
    }
    fn fail(toks: &[Token], err: ParseError) {
        let iter = toks.iter().cloned().peekable();
        let res = Parser::new(iter)
            .collect::<ParseResult<Vec<_>>>()
            .unwrap_err();
        assert_eq!(err, res.1);
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
    fn semicolon_error() {
        fail(&toklist!["a"], ParseError::Expected(&[TokType::Semicolon]));
    }
    // TODO: add more tests
}
