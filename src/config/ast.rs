#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Entry {
    pub left: Spec,
    pub right: Option<Spec>,
}

// A `Spec` specifies a fragment of a path, e.g. "~/.config/[nvim/init.vim, spectrwm.conf]".
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Spec {
    pub string: Option<String>,
    pub spectype: SpecType,
}
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum SpecType {
    None,
    Variant(Box<VariantExpr>, Option<Box<Spec>>),
    Match(Box<MatchExpr>, Option<Box<Spec>>),
}
impl Spec {
    // Returns None if the nr. of options is larger than usize::MAX.
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
}
impl From<String> for Spec {
    fn from(s: String) -> Self {
        Spec {
            string: Some(s),
            spectype: SpecType::None,
        }
    }
}
impl From<&str> for Spec {
    fn from(s: &str) -> Self {
        Spec {
            string: Some(s.to_owned()),
            spectype: SpecType::None,
        }
    }
}
impl SpecType {
    pub fn variant_expr(specs: Vec<Spec>, rest: Option<Spec>) -> Self {
        SpecType::Variant(Box::new(VariantExpr { specs }), rest.map(Box::new))
    }
    pub fn match_expr(cases: Vec<(Expr, Spec)>, rest: Option<Spec>) -> Self {
        SpecType::Match(Box::new(MatchExpr { cases }), rest.map(Box::new))
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct VariantExpr {
    pub specs: Vec<Spec>,
}
impl VariantExpr {
    // Returns None if the nr. of options is larger than usize::MAX.
    pub fn nr_of_options(&self) -> Option<usize> {
        self.specs.iter().try_fold(0usize, |nr, spec| {
            spec.nr_of_options()
                .and_then(|specnr| specnr.checked_add(nr))
        })
    }
}

// Matches, based on the expr, which spec to produce.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct MatchExpr {
    pub cases: Vec<(Expr, Spec)>,
}
impl MatchExpr {
    pub fn resolve(&self) -> Option<&Spec> {
        use std::env::consts::OS;
        let os = match OS {
            "linux" => Some(ExprType::Linux),
            "windows" => Some(ExprType::Windows),
            "macos" => Some(ExprType::Macos),
            "freebsd" | "netbsd" | "openbsd" => Some(ExprType::Bsd),
            _ => None,
        };
        for case in &self.cases {
            if (cfg!(unix) && case.0.exprtype == ExprType::Unix) // is unix
                || os.as_ref().map(|x| *x == case.0.exprtype).unwrap_or(false) // OS matches the expr
                || case.0.exprtype == ExprType::Any
            // this expr is the default one
            {
                // it matches
                return Some(&case.1);
            }
        }
        None
    }
}

// Something that is either true or false, depending on the system.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Expr {
    pub exprtype: ExprType,
}
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum ExprType {
    Windows,
    Linux,
    Macos,
    Unix,
    Bsd,
    Any,
}
impl From<ExprType> for Expr {
    fn from(exprtype: ExprType) -> Expr {
        Expr { exprtype }
    }
}
