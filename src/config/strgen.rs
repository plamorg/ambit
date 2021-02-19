use crate::config::ast::*;

use std::rc::Rc;

// Restarts an iterator.
trait Restartable
where
    Self: Iterator + std::fmt::Debug,
{
    fn restart(&mut self);
}

/* A binary tree that only stores
 * information in its leaf nodes.
 * Can be thought of as "nested pairs of items",
 * e.g. (("x", ("a", "r")), "y").
 * (This exists to increase the efficiency
 * of iteration for a Spec.)
 */
#[derive(Debug, PartialEq, Eq, Clone)]
enum PairTree<T> {
    Value(T),
    Pair(Box<PairTree<T>>, Box<PairTree<T>>),
    // Exists to allow `Rc` nodes in the tree.
    // (Used if a pre-calculated value must be iterated
    // through again.)
    Rc(Rc<PairTree<T>>),
}
impl<T> PairTree<T> {
    pub fn value(v: T) -> Self {
        PairTree::Value(v)
    }
    pub fn pair(left: PairTree<T>, right: PairTree<T>) -> Self {
        PairTree::Pair(Box::new(left), Box::new(right))
    }
    pub fn rc(tree: &Rc<PairTree<T>>) -> Self {
        PairTree::Rc(Rc::clone(tree))
    }
}
impl<'a> PairTree<&'a str> {
    pub fn flatten_to_string(&self) -> String {
        fn get_total_length(tree: &PairTree<&str>) -> usize {
            match tree {
                PairTree::Value(val) => val.len(),
                PairTree::Pair(left, right) => get_total_length(left) + get_total_length(right),
                PairTree::Rc(rc_val) => get_total_length(rc_val),
            }
        }
        fn construct_string(tree: &PairTree<&str>, result_string: &mut String) {
            match tree {
                PairTree::Value(val) => *result_string += val,
                PairTree::Pair(left, right) => {
                    construct_string(left, result_string);
                    construct_string(right, result_string);
                }
                PairTree::Rc(tree) => construct_string(tree, result_string),
            }
        }
        let mut result = String::with_capacity(get_total_length(self));
        construct_string(self, &mut result);
        result
    }
}
impl<T> From<T> for PairTree<T> {
    fn from(s: T) -> Self {
        Self::Value(s)
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
        let tree = self.iter.next()?;
        Some(tree.flatten_to_string())
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
    should_emit_string: bool,
}
impl<'a> SpecIter<'a> {
    pub fn new(spec: &'a Spec) -> Self {
        let mut ret = SpecIter {
            spec,
            curr_expr: None,
            expr_iter: None,
            spec_iter: None,
            should_emit_string: true,
        };
        ret.init_expr_iter();
        ret.init_spec_iter();
        ret
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
        let expr_iter = self.expr_iter.as_mut().expect("expr_iter must exist");
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
        self.should_emit_string = true;
    }
}
impl<'a> Iterator for SpecIter<'a> {
    type Item = PairTree<&'a str>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.spec.spectype == SpecType::None {
            return if self.should_emit_string {
                self.should_emit_string = false;
                self.spec
                    .string
                    .as_ref()
                    .map(|x| PairTree::value(x.as_str()))
            } else {
                None
            };
        }
        self.next_without_str().map(|rest| match &self.spec.string {
            Some(s) => PairTree::pair(s.as_str().into(), rest),
            None => rest,
        })
    }
}

#[derive(Debug)]
struct VariantIter<'a> {
    expr: &'a VariantExpr,
    // The current variant's iterator.
    curr_iter: Option<Box<SpecIter<'a>>>,
    // The index after the current variant.
    index: usize,
}
impl<'a> Restartable for VariantIter<'a> {
    fn restart(&mut self) {
        self.curr_iter = None;
        self.index = 0;
    }
}
impl<'a> Iterator for VariantIter<'a> {
    type Item = PairTree<&'a str>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Check if the current variant's iterator exists and is producing something.
            if let Some(curr_result) = self.curr_iter.as_mut().and_then(|iter| iter.next()) {
                return Some(curr_result);
            } else {
                // This variant's iterator is finished.
                if self.index >= self.expr.specs.len() {
                    // We have no more variants to go through.
                    return None;
                }
                // Advance to the next variant's iterator.
                self.curr_iter = Some(Box::new(self.expr.specs[self.index].raw_iter()));
                self.index += 1;
            }
        }
    }
}

impl VariantExpr {
    fn raw_iter(&self) -> VariantIter {
        VariantIter {
            expr: self,
            curr_iter: None,
            index: 0,
        }
    }
}

#[derive(Debug)]
struct MatchIter<'a> {
    expr: &'a MatchExpr,
    spec_iter: Option<SpecIter<'a>>,
}
impl<'a> Iterator for MatchIter<'a> {
    type Item = PairTree<&'a str>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.spec_iter.as_mut() {
            Some(iter) => iter.next(),
            None => None,
        }
    }
}
impl Restartable for MatchIter<'_> {
    fn restart(&mut self) {
        if let Some(iter) = self.spec_iter.as_mut() {
            iter.restart();
        }
    }
}
impl MatchExpr {
    fn raw_iter(&self) -> MatchIter {
        MatchIter {
            expr: &self,
            spec_iter: match self.resolve() {
                Some(spec) => Some(spec.raw_iter()),
                None => None,
            },
        }
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
        results_in(Spec::from("abc"), vec!["abc"]);
    }

    #[test]
    fn basic_variant() {
        results_in(
            Spec {
                string: Some("a".to_owned()),
                spectype: SpecType::variant_expr(vec![Spec::from("b"), Spec::from("c")], None),
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
                spectype: SpecType::match_expr(
                    vec![
                        (Expr::incorrect_os(), Spec::from("g")),
                        (ExprType::Any.into(), Spec::from("e")),
                    ],
                    Some(Spec::from("f")),
                ),
            },
            vec!["def"],
        )
    }

    #[test]
    fn unresolvable_match() {
        results_in(
            // Equivalent to `d{ incorrect-os: g, }f`.
            Spec {
                string: Some("d".to_owned()),
                spectype: SpecType::match_expr(
                    vec![(Expr::incorrect_os(), Spec::from("g"))],
                    Some(Spec::from("f")),
                ),
            },
            // Since the MatchExpr can't resolve to anything,
            // there is nothing here.
            // (At least, if the test _succeeds_.)
            vec![],
        )
    }

    #[test]
    fn nested_variant() {
        results_in(
            // Equivalent to `a[b, c[d[e, f], g], h]i`.
            Spec {
                string: Some("a".to_owned()),
                spectype: SpecType::variant_expr(
                    vec![
                        Spec::from("b"),
                        Spec {
                            string: Some("c".to_owned()),
                            spectype: SpecType::variant_expr(
                                vec![
                                    Spec {
                                        string: Some("d".to_owned()),
                                        spectype: SpecType::variant_expr(
                                            vec![Spec::from("e"), Spec::from("f")],
                                            None,
                                        ),
                                    },
                                    Spec::from("g"),
                                ],
                                None,
                            ),
                        },
                        Spec::from("h"),
                    ],
                    Some(Spec::from("i")),
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
                spectype: SpecType::variant_expr(
                    vec![Spec::from("a"), Spec::from("b"), Spec::from("c")],
                    Some(Spec {
                        spectype: SpecType::variant_expr(
                            vec![Spec::from("d"), Spec::from("e"), Spec::from("f")],
                            Some(Spec {
                                string: None,
                                spectype: SpecType::variant_expr(
                                    vec![Spec::from("g"), Spec::from("h"), Spec::from("i")],
                                    None,
                                ),
                            }),
                        ),
                        string: None,
                    }),
                ),
            },
            res_vec_str,
        );
    }

    // TODO: add more tests
}
