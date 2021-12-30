use std::fmt::Display;
use std::ops::{Neg, Add, Sub};

#[derive(Clone,PartialEq,Copy,Eq)]
pub enum Value {
    MATE(i32),
    CENTIS(i32),
    NEGINFTY,
    INFTY,
}

#[derive(Clone,PartialEq,Copy,Eq)]
pub enum Bound {
    EXACT,
    UPPERBOUND,
    LOWERBOUND,
}

#[derive(Clone,PartialEq,Copy,Eq)]
pub struct Eval {
    bound: Bound,
    value: Value,
}


impl Eval {
    pub const MIN: Eval = Eval {bound: Bound::EXACT, value: Value::NEGINFTY};
    pub const MAX: Eval = Eval {bound: Bound::EXACT, value: Value::INFTY};
    pub const MATE_NOW: Eval = Eval {bound: Bound::EXACT, value: Value::MATE(0)};
    pub const STALEMATE: Eval = Eval {bound: Bound::EXACT, value: Value::CENTIS(0)};
    pub const DRAW: Eval = Eval {bound: Bound::EXACT, value: Value::CENTIS(0)};

    pub fn new(bound: Bound, value: Value) -> Eval {
        Eval {bound, value}
    }

    pub fn mate_in(moves: i32) -> Eval {
        Eval {bound: Bound::EXACT, value: Value::MATE(moves)}
    }

    #[allow(dead_code)]
    pub fn exact_from_cents(centis: i32) -> Eval {
        Eval {bound: Bound::EXACT, value: Value::CENTIS(centis)}
    }

    #[allow(dead_code)]
    pub fn lowerbound_from_cents(centis: i32) -> Eval {
        Eval {bound: Bound::LOWERBOUND, value: Value::CENTIS(centis)}
    }

    #[allow(dead_code)]
    pub fn upperbound_from_cents(centis: i32) -> Eval {
        Eval {bound: Bound::UPPERBOUND, value: Value::CENTIS(centis)}
    }

    #[inline(always)]
    pub fn to_exact(self) -> Eval {
        Eval {bound: Bound::EXACT, value: self.value}
    }

    #[inline(always)]
    pub fn to_lowerbound(self) -> Eval {
        Eval {bound: Bound::LOWERBOUND, value: self.value}
    }

    #[inline(always)]
    pub fn to_upperbound(self) -> Eval {
        Eval {bound: Bound::UPPERBOUND, value: self.value}
    }

    const ASPIRATION_ADJUSTMENTS: [i32; 5] = [25, 50, 200, 400, 800];

    pub fn aspiration_lower(&self, count: usize) -> Eval {
        if count >= 5 {
            return Eval{bound: Bound::EXACT, value: Value::NEGINFTY};
        }
        match self.value {
            Value::CENTIS(c) => Eval { bound: self.bound, value: Value::CENTIS(c-Eval::ASPIRATION_ADJUSTMENTS[count]) },
            //In case of mate take the next worse mate score, e.g. mate in 2 if we are being mated
            //in 3 or mate in 5 if we will mate in 3
            Value::MATE(m) => {
                let mut val = Value::NEGINFTY;
                //We are mating, so the next worse thing is mating slower
                if m % 2 == 1 {
                    val = Value::MATE(m+2);
                } else if m > 1 { //We are getting mated so we aspire to do so faster, if at all possible
                    val = Value::MATE(m-2);
                }
                Eval{ bound: self.bound, value: val }
            },
            _ => Eval{bound: Bound::EXACT, value: self.value}
        }
    }

    pub fn aspiration_higher(&self, count: usize) -> Eval {
        if count >= 5 {
            return Eval{bound: Bound::EXACT, value: Value::INFTY};
        }
        match self.value {
            Value::CENTIS(c) => Eval { bound: self.bound, value: Value::CENTIS(c+Eval::ASPIRATION_ADJUSTMENTS[count]) },
            //In case of mate take the next best mate score, e.g. mate in 4 if we are being mated
            //in 2 or mate in 3 if we will mate in 5
            Value::MATE(m) => {
                let mut val = Value::INFTY;
                //We are getting mated, so the next best thing is getting mated slower
                if m % 2 == 0 {
                    val = Value::MATE(m+2);
                } else if m > 1 { //We are mating so we aspire to do so faster, if at all possible
                    val = Value::MATE(m-2);
                }
                Eval{ bound: self.bound, value: val }
            },
            _ => Eval{bound: Bound::EXACT, value: Value::INFTY}
        }

    }

    #[inline(always)]
    pub fn zero_window(&self) -> Eval {
        match self.value {
            Value::CENTIS(c) => Eval { value: Value::CENTIS(c+1), bound: self.bound },
            Value::MATE(_) => {
                self.aspiration_higher(0)
            },
            Value::INFTY => *self,
            Value::NEGINFTY => *self,
        }
    }

    //Use to pass ab-bounds down the tree
    #[inline(always)]
    pub fn neg_down(&self) -> Eval {
        let val = match self.value {
            Value::MATE(m) => if m == 0 {Value::INFTY} else {Value::MATE(m-1)},
            Value::CENTIS(c) => Value::CENTIS(-c),
            Value::NEGINFTY => Value::INFTY,
            Value::INFTY => Value::NEGINFTY,
        };
        Eval {value: val, bound: self.bound}
    }

    #[inline(always)]
    pub fn value(&self) -> Value {
        self.value
    }

    #[inline(always)]
    pub fn bound(&self) -> Bound {
        self.bound
    }
}

impl Display for Eval {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f,"score ")?;
        match self.value {
            Value::CENTIS(c) => write!(f, "cp {}", c),
            Value::INFTY => write!(f, "cp 100000000"),
            Value::NEGINFTY => write!(f, "cp -100000000"),
            Value::MATE(m) =>write!(f, "mate {}", (2*(m%2)-1)*(m+1)/2),
        }?;
        match self.bound {
            Bound::LOWERBOUND => write!(f," lowerbound"),
            Bound::UPPERBOUND => write!(f," upperbound"),
            Bound::EXACT => write!(f,""),
        }
    }
}

//Neg moves a result UP the searchtree, i.e. mates become father away. Use Eval::neg_down
impl Neg for Value {
    type Output = Self;
    fn neg(self) -> Self {
        match self {
            Self::MATE(m) => Self::MATE(m+1),
            Self::CENTIS(c) => Self::CENTIS(-c),
            Self::NEGINFTY => Self::INFTY,
            Self::INFTY => Self::NEGINFTY,
        }
    }
}

impl Neg for Eval {
    type Output = Eval;
    fn neg(self) -> Self {
        Eval {
            bound: match self.bound {
                Bound::EXACT => Bound::EXACT,
                Bound::UPPERBOUND => Bound::LOWERBOUND,
                Bound::LOWERBOUND => Bound::UPPERBOUND,
            },
            value: -self.value,
        }
    }
}

impl Ord for Eval {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialOrd for Eval {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering>{
        match self.value {
            Value::MATE(m) => match other.value {
                Value::MATE(m2) => {
                    //Note that we have m2.cmp(m), since mate in 3 plys is better than mate in
                    //5 plys
                    if m % 2 == 1 && m2 % 2 == 1 { Some(m2.cmp(&m)) }
                    else if m % 2 == 1 && m2 % 2 == 0 { Some(std::cmp::Ordering::Greater) }
                    else if m % 2 == 0 && m2 % 2 == 1 { Some(std::cmp::Ordering::Less) }
                    //If we get mated, a long time off is best!
                    else { Some(m.cmp(&m2)) }
                }
                Value::INFTY => Some(std::cmp::Ordering::Less),
                Value::NEGINFTY => Some(std::cmp::Ordering::Greater),
                Value::CENTIS(_) => if m % 2 == 1 { Some(std::cmp::Ordering::Greater) } else { Some(std::cmp::Ordering::Less) },
            },
            Value::INFTY => match other.value {
                Value::INFTY => Some(std::cmp::Ordering::Equal),
                _ => Some(std::cmp::Ordering::Greater),
            },
            Value::NEGINFTY => match other.value {
                Value::NEGINFTY => Some(std::cmp::Ordering::Equal),
                _ => Some(std::cmp::Ordering::Less),
            },
            Value::CENTIS(c) => match other.value {
                Value::MATE(m) => if m % 2 == 1 { Some(std::cmp::Ordering::Less) } else { Some(std::cmp::Ordering::Greater) },
                Value::INFTY => Some(std::cmp::Ordering::Less),
                Value::NEGINFTY => Some(std::cmp::Ordering::Greater),
                Value::CENTIS(c2) => Some(c.cmp(&c2)),
            }
        }
    }
}

impl Add<i32> for Eval {
    type Output = Self;
    fn add(self, rhs: i32) -> Self::Output {
        match self.value {
            Value::CENTIS(c) => Eval {value: Value::CENTIS(rhs+c), bound: self.bound},
            _ => self,
        }
    }
}

impl Sub<i32> for Eval {
    type Output = Self;
    fn sub(self, rhs: i32) -> Self::Output {
        match self.value {
            Value::CENTIS(c) => Eval {value: Value::CENTIS(c-rhs), bound: self.bound},
            _ => self,
        }
    }
}

//Tests
#[test]
fn ab_comp_test() {
    assert!(Eval::MIN < Eval::MAX);
    assert!(Eval::MIN < Eval::MATE_NOW);
    assert!(Eval::MATE_NOW < Eval::MAX);
    assert!(Eval::MATE_NOW < Eval::exact_from_cents(-100));
    assert!(Eval::exact_from_cents(-100) < Eval::exact_from_cents(100));
    assert!(Eval::exact_from_cents(100) < -Eval::MATE_NOW);
    assert!(Eval::MATE_NOW < -Eval::MATE_NOW);
    assert!(-Eval::MATE_NOW > Eval::MATE_NOW);
    assert!(-Eval::MATE_NOW == Eval{bound: Bound::EXACT, value: Value::MATE(1)});
    assert!(-(-(-Eval::MATE_NOW)) == Eval{bound: Bound::EXACT, value: Value::MATE(3)});
}

