use crate::config::ast::*;

use std::rc::Rc;

// Restarts an iterator.
trait Restartable
where
    Self: Iterator + std::fmt::Debug,
{
    fn restart(&mut self);
}

/* A tree of pairs.
 * (This exists to increase the efficiency
 * of iteration for a Spec.)
 */
#[derive(Debug, PartialEq, Eq, Clone)]
enum PairTree<T> {
    Val(T),
    Pair(Box<PairTree<T>>, Box<PairTree<T>>),
    Rc(Rc<PairTree<T>>),
}
impl<T> PairTree<T> {
    pub fn val(v: T) -> Self {
        PairTree::Val(v)
    }
    pub fn pair(left: PairTree<T>, right: PairTree<T>) -> Self {
        PairTree::Pair(Box::new(left), Box::new(right))
    }
    pub fn rc(tree: &Rc<PairTree<T>>) -> Self {
        PairTree::Rc(Rc::clone(tree))
    }
}
impl<T> From<T> for PairTree<T> {
    fn from(s: T) -> Self {
        Self::Val(s)
    }
}
impl<T> From<(Box<PairTree<T>>, Box<PairTree<T>>)> for PairTree<T> {
    fn from(pair: (Box<PairTree<T>>, Box<PairTree<T>>)) -> Self {
        Self::Pair(pair.0, pair.1)
    }
}

pub struct SpecStrIter<'a> {
    iter: SpecIter<'a>,
}
impl<'a> Iterator for SpecStrIter<'a> {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        fn get_str_size(tree: &PairTree<&str>) -> usize {
            match tree {
                PairTree::Pair(left, right) => get_str_size(left) + get_str_size(right),
                PairTree::Rc(tree) => get_str_size(tree),
                PairTree::Val(s) => s.len(),
            }
        }
        fn construct_str(ret: &mut String, tree: &PairTree<&str>) {
            match tree {
                PairTree::Pair(left, right) => {
                    construct_str(ret, left);
                    construct_str(ret, right);
                }
                PairTree::Rc(tree) => construct_str(ret, tree),
                PairTree::Val(s) => *ret += s,
            }
        }
        let tree = self.iter.next()?;
        let mut ret = String::new();
        ret.reserve(get_str_size(&tree));
        construct_str(&mut ret, &tree);
        Some(ret)
    }
}

impl<'a> IntoIterator for &'a Spec {
    type Item = String;
    type IntoIter = SpecStrIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        SpecStrIter {
            iter: SpecIter::new(self),
        }
    }
}

impl Spec {
    fn raw_iter(&self) -> SpecIter {
        SpecIter::new(self)
    }
}

#[derive(Debug)]
struct SpecIter<'a> {
    spec: &'a Spec,
    expr_iter: Option<Box<dyn Restartable<Item = PairTree<&'a str>> + 'a>>,
    curr_expr: Option<Rc<PairTree<&'a str>>>,
    spec_iter: Option<Box<SpecIter<'a>>>,
    string_emitted: bool,
}
impl<'a> SpecIter<'a> {
    pub fn new(spec: &'a Spec) -> Self {
        let mut ret = SpecIter {
            spec,
            curr_expr: None,
            expr_iter: None,
            spec_iter: None,
            string_emitted: false,
        };
        ret.init();
        ret
    }
    fn init(&mut self) {
        self.init_expr_iter();
        self.init_spec_iter();
    }
    fn init_expr_iter(&mut self) {
        self.expr_iter = match &self.spec.spectype {
            SpecType::None => None,
            SpecType::Match(expr, _) => Some(Box::new(expr.raw_iter())),
            SpecType::Variant(expr, _) => Some(Box::new(expr.raw_iter())),
        }
    }
    fn init_spec_iter(&mut self) {
        self.spec_iter = match &self.spec.spectype {
            SpecType::None => None,
            SpecType::Match(_, next_spec) | SpecType::Variant(_, next_spec) => {
                next_spec.as_ref().map(|spec| Box::new(spec.raw_iter()))
            }
        }
    }
    // Returns the next item, not considering `self.spec.string`.
    fn next_without_str(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.spec.spectype == SpecType::None {
            return None;
        }
        let expr_iter = self
            .expr_iter
            .as_mut()
            .expect("expr_iter must be accessible");
        if let Some(spec_iter) = self.spec_iter.as_mut() {
            // We have to deal with the rest of this Spec.
            loop {
                if let Some(curr_expr) = self.curr_expr.as_ref() {
                    if let Some(rest) = spec_iter.next() {
                        return Some(PairTree::pair(PairTree::rc(&curr_expr), rest));
                    } else {
                        // We need to restart the "fast" spec_iter,
                        // and therefore (by exiting the if statement)
                        // also advance the "slow" expr_iter.
                        spec_iter.restart();
                    }
                }
                // If the curr_expr needs refreshing, do so.
                self.curr_expr = Some(Rc::new(expr_iter.next()?));
            }
        } else {
            // We don't have a further Spec to deal with,
            // just a simple iterator over the other values will do.
            Some(PairTree::rc(&Rc::new(expr_iter.next()?)))
        }
    }
}
impl<'a> Restartable for SpecIter<'a> {
    fn restart(&mut self) {
        if let Some(expr_iter) = self.expr_iter.as_mut() {
            expr_iter.restart();
        }
        if let Some(spec_iter) = self.spec_iter.as_mut() {
            spec_iter.restart();
        }
        self.curr_expr = None;
        self.string_emitted = false;
    }
}
impl<'a> Iterator for SpecIter<'a> {
    type Item = PairTree<&'a str>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.spec.spectype == SpecType::None {
            return if !self.string_emitted {
                self.string_emitted = true;
                self.spec.string.as_ref().map(|x| PairTree::val(x.as_str()))
            } else {
                None
            };
        }
        self.next_without_str().and_then(|rest| {
            Some(match &self.spec.string {
                Some(s) => PairTree::pair(s.as_str().into(), rest),
                None => rest,
            })
        })
    }
}

#[derive(Debug)]
struct VariantIter<'a> {
    expr: &'a VariantExpr,
    curr: Option<Box<SpecIter<'a>>>,
    idx: usize,
}
impl<'a> Restartable for VariantIter<'a> {
    fn restart(&mut self) {
        self.curr = None;
        self.idx = 0;
    }
}
impl<'a> Iterator for VariantIter<'a> {
    type Item = PairTree<&'a str>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ret) = self.curr.as_mut().and_then(|iter| iter.next()) {
                return Some(ret);
            } else {
                // This variant's iterator is finished, move on to the next one.
                if self.idx >= self.expr.specs.len() {
                    // We're completely done.
                    return None;
                }
                self.curr = Some(Box::new(self.expr.specs[self.idx].raw_iter()));
                self.idx += 1;
            }
        }
    }
}

impl VariantExpr {
    fn raw_iter(&self) -> VariantIter {
        VariantIter {
            expr: self,
            curr: None,
            idx: 0,
        }
    }
}

impl MatchExpr {
    fn raw_iter(&self) -> SpecIter {
        self.resolve().raw_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results_in(spec: Spec, expected: Vec<&str>) {
        let yielded: Vec<_> = spec.into_iter().collect();
        assert_eq!(yielded, expected);
    }

    #[test]
    fn basic_string() {
        results_in(
            Spec {
                spectype: SpecType::None,
                string: Some("abc".to_owned()),
            },
            vec!["abc"],
        );
    }

    #[test]
    fn basic_variant() {
        results_in(
            Spec {
                string: Some("a".to_owned()),
                spectype: SpecType::Variant(
                    Box::new(VariantExpr {
                        specs: vec![
                            Spec {
                                spectype: SpecType::None,
                                string: Some("b".to_owned()),
                            },
                            Spec {
                                spectype: SpecType::None,
                                string: Some("c".to_owned()),
                            },
                        ],
                    }),
                    None,
                ),
            },
            vec!["ab", "ac"],
        )
    }

    #[test]
    fn basic_match() {
        results_in(
            // Equivalent to `d{ incorrect-os: g, default: e }f`.
            Spec {
                string: Some("d".to_owned()),
                spectype: SpecType::Match(
                    Box::new(MatchExpr {
                        cases: vec![(
                            Expr {
                                exprtype: if cfg!(windows) {
                                    ExprType::Linux
                                } else {
                                    ExprType::Windows
                                },
                            },
                            Spec {
                                string: Some("g".to_owned()),
                                spectype: SpecType::None,
                            },
                        )],
                        default: Spec {
                            string: Some("e".to_owned()),
                            spectype: SpecType::None,
                        },
                    }),
                    Some(Box::new(Spec {
                        string: Some("f".to_owned()),
                        spectype: SpecType::None,
                    })),
                ),
            },
            vec!["def"],
        )
    }

    #[test]
    fn nested_variant() {
        results_in(
            // Equivalent to `a[b, c[d[e, f], g], h]i`.
            Spec {
                string: Some("a".to_owned()),
                spectype: SpecType::Variant(
                    Box::new(VariantExpr {
                        specs: vec![
                            Spec {
                                string: Some("b".to_owned()),
                                spectype: SpecType::None,
                            },
                            Spec {
                                string: Some("c".to_owned()),
                                spectype: SpecType::Variant(
                                    Box::new(VariantExpr {
                                        specs: vec![
                                            Spec {
                                                string: Some("d".to_owned()),
                                                spectype: SpecType::Variant(
                                                    Box::new(VariantExpr {
                                                        specs: vec![
                                                            Spec {
                                                                string: Some("e".to_string()),
                                                                spectype: SpecType::None,
                                                            },
                                                            Spec {
                                                                string: Some("f".to_string()),
                                                                spectype: SpecType::None,
                                                            },
                                                        ],
                                                    }),
                                                    None,
                                                ),
                                            },
                                            Spec {
                                                string: Some("g".to_string()),
                                                spectype: SpecType::None,
                                            },
                                        ],
                                    }),
                                    None,
                                ),
                            },
                            Spec {
                                string: Some("h".to_string()),
                                spectype: SpecType::None,
                            },
                        ],
                    }),
                    Some(Box::new(Spec {
                        string: Some("i".to_owned()),
                        spectype: SpecType::None,
                    })),
                ),
            },
            vec!["abi", "acdei", "acdfi", "acgi", "ahi"],
        )
    }

    #[test]
    fn adjacent_variants() {
        let mut res_vec = Vec::new();
        for i in ['a', 'b', 'c'].iter() {
            for j in ['d', 'e', 'f'].iter() {
                for k in ['g', 'h', 'i'].iter() {
                    let s: String = [*i, *j, *k].iter().collect();
                    res_vec.push(s);
                }
            }
        }
        let res_vec_str = res_vec.iter().map(|x| x.as_str()).collect();
        results_in(
            // Equivalent to `[a,b,c][d,e,f][g,h,i]`.
            Spec {
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
                            Spec {
                                string: Some("c".to_owned()),
                                spectype: SpecType::None,
                            },
                        ],
                    }),
                    Some(Box::new(Spec {
                        spectype: SpecType::Variant(
                            Box::new(VariantExpr {
                                specs: vec![
                                    Spec {
                                        string: Some("d".to_owned()),
                                        spectype: SpecType::None,
                                    },
                                    Spec {
                                        string: Some("e".to_owned()),
                                        spectype: SpecType::None,
                                    },
                                    Spec {
                                        string: Some("f".to_owned()),
                                        spectype: SpecType::None,
                                    },
                                ],
                            }),
                            Some(Box::new(Spec {
                                string: None,
                                spectype: SpecType::Variant(
                                    Box::new(VariantExpr {
                                        specs: vec![
                                            Spec {
                                                string: Some("g".to_owned()),
                                                spectype: SpecType::None,
                                            },
                                            Spec {
                                                string: Some("h".to_owned()),
                                                spectype: SpecType::None,
                                            },
                                            Spec {
                                                string: Some("i".to_owned()),
                                                spectype: SpecType::None,
                                            },
                                        ],
                                    }),
                                    None,
                                ),
                            })),
                        ),
                        string: None,
                    })),
                ),
            },
            res_vec_str,
        );
    }

    // TODO: add more tests
}
