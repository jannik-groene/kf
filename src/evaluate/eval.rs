use std::fmt::Display;
use std::ops::{Add, Neg, Sub};

#[derive(Clone, PartialEq, Copy, Eq, Debug)]
pub enum Value {
    Mate(i32),
    Centis(i32),
    NegInfty,
    Infty,
}

#[derive(Clone, PartialEq, Copy, Eq)]
pub enum Bound {
    Exact,
    Upper,
    Lower,
}

#[derive(Clone, PartialEq, Copy, Eq)]
pub struct Eval {
    bound: Bound,
    value: Value,
}

impl Eval {
    pub const MIN: Eval = Eval {
        bound: Bound::Exact,
        value: Value::NegInfty,
    };
    pub const MAX: Eval = Eval {
        bound: Bound::Exact,
        value: Value::Infty,
    };
    pub const MATE_NOW: Eval = Eval {
        bound: Bound::Exact,
        value: Value::Mate(0),
    };
    pub const STALEMATE: Eval = Eval {
        bound: Bound::Exact,
        value: Value::Centis(0),
    };
    pub const DRAW: Eval = Eval {
        bound: Bound::Exact,
        value: Value::Centis(0),
    };

    pub fn new(bound: Bound, value: Value) -> Eval {
        Eval { bound, value }
    }

    pub fn mate_in(moves: i32) -> Eval {
        Eval {
            bound: Bound::Exact,
            value: Value::Mate(moves),
        }
    }

    #[allow(dead_code)]
    pub fn exact_from_cents(centis: i32) -> Eval {
        Eval {
            bound: Bound::Exact,
            value: Value::Centis(centis),
        }
    }

    #[allow(dead_code)]
    pub fn lowerbound_from_cents(centis: i32) -> Eval {
        Eval {
            bound: Bound::Lower,
            value: Value::Centis(centis),
        }
    }

    #[allow(dead_code)]
    pub fn upperbound_from_cents(centis: i32) -> Eval {
        Eval {
            bound: Bound::Upper,
            value: Value::Centis(centis),
        }
    }

    #[inline(always)]
    pub fn to_exact(self) -> Eval {
        Eval {
            bound: Bound::Exact,
            value: self.value,
        }
    }

    #[inline(always)]
    pub fn to_lowerbound(self) -> Eval {
        Eval {
            bound: Bound::Lower,
            value: self.value,
        }
    }

    #[inline(always)]
    pub fn to_upperbound(self) -> Eval {
        Eval {
            bound: Bound::Upper,
            value: self.value,
        }
    }

    const ASPIRATION_ADJUSTMENTS: [i32; 5] = [25, 50, 200, 400, 800];

    pub fn aspiration_lower(&self, count: usize) -> Eval {
        if count >= 5 {
            return Eval {
                bound: Bound::Exact,
                value: Value::NegInfty,
            };
        }
        match self.value {
            Value::Centis(c) => Eval {
                bound: self.bound,
                value: Value::Centis(c - Eval::ASPIRATION_ADJUSTMENTS[count]),
            },
            //In case of mate take the next worse mate score, e.g. mate in 2 if we are being mated
            //in 3 or mate in 5 if we will mate in 3
            Value::Mate(m) => {
                let mut val = Value::NegInfty;
                //We are mating, so the next worse thing is mating slower
                if m % 2 == 1 {
                    val = Value::Mate(m + 2);
                } else if m > 1 {
                    //We are getting mated so we aspire to do so faster, if at all possible
                    val = Value::Mate(m - 2);
                }
                Eval {
                    bound: self.bound,
                    value: val,
                }
            }
            _ => Eval {
                bound: Bound::Exact,
                value: self.value,
            },
        }
    }

    pub fn aspiration_higher(&self, count: usize) -> Eval {
        if count >= 5 {
            return Eval {
                bound: Bound::Exact,
                value: Value::Infty,
            };
        }
        match self.value {
            Value::Centis(c) => Eval {
                bound: self.bound,
                value: Value::Centis(c + Eval::ASPIRATION_ADJUSTMENTS[count]),
            },
            //In case of mate take the next best mate score, e.g. mate in 4 if we are being mated
            //in 2 or mate in 3 if we will mate in 5
            Value::Mate(m) => {
                let mut val = Value::Infty;
                //We are getting mated, so the next best thing is getting mated slower
                if m % 2 == 0 {
                    val = Value::Mate(m + 2);
                } else if m > 1 {
                    //We are mating so we aspire to do so faster, if at all possible
                    val = Value::Mate(m - 2);
                }
                Eval {
                    bound: self.bound,
                    value: val,
                }
            }
            _ => Eval {
                bound: Bound::Exact,
                value: Value::Infty,
            },
        }
    }

    #[inline(always)]
    pub fn zero_window(&self) -> Eval {
        match self.value {
            Value::Centis(c) => Eval {
                value: Value::Centis(c + 1),
                bound: self.bound,
            },
            Value::Mate(_) => self.aspiration_higher(0),
            Value::Infty => *self,
            Value::NegInfty => *self,
        }
    }

    //Use to pass ab-bounds down the tree
    #[inline(always)]
    pub fn neg_down(&self) -> Eval {
        let val = match self.value {
            Value::Mate(m) => {
                if m == 0 {
                    Value::Infty
                } else {
                    Value::Mate(m - 1)
                }
            }
            Value::Centis(c) => Value::Centis(-c),
            Value::NegInfty => Value::Infty,
            Value::Infty => Value::NegInfty,
        };
        Eval {
            value: val,
            bound: self.bound,
        }
    }

    #[inline(always)]
    pub fn value(&self) -> Value {
        self.value
    }

    #[inline(always)]
    pub fn bound(&self) -> Bound {
        self.bound
    }

    pub fn pack_for_tt(&self) -> u64 {
        let btype = match self.bound {
            Bound::Lower => 0,
            Bound::Upper => 1,
            Bound::Exact => 2,
        };
        let (kind,value) = match self.value {
            Value::Centis(c) => (0,c),
            Value::Mate(n)   => (1,n),
            Value::Infty     => (2,0),
            Value::NegInfty  => (3,0),
        };
        btype ^ (kind << 2) ^ ((value as u64) << 4)
    }

    pub fn from_packed(val: u64) -> Eval {
        let bound = match val & 0b11 {
            0 => Bound::Lower,
            1 => Bound::Upper,
            _ => Bound::Exact,
        };
        let value = match (val >> 2) & 0b11 {
            0 => Value::Centis((val >> 4) as i32),
            1 => Value::Mate((val >> 4) as i32),
            2 => Value::Infty,
            _ => Value::NegInfty,
        };
        Eval {
            bound,
            value,
        }
    }
}

impl Display for Eval {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "score ")?;
        match self.value {
            Value::Centis(c) => write!(f, "cp {}", c),
            Value::Infty => write!(f, "cp 100000000"),
            Value::NegInfty => write!(f, "cp -100000000"),
            Value::Mate(m) => write!(f, "mate {}", (2 * (m % 2) - 1) * (m + 1) / 2),
        }?;
        match self.bound {
            Bound::Lower => write!(f, " lowerbound"),
            Bound::Upper => write!(f, " upperbound"),
            Bound::Exact => write!(f, ""),
        }
    }
}

//Neg moves a result UP the searchtree, i.e. mates become father away. Use Eval::neg_down
impl Neg for Value {
    type Output = Self;
    fn neg(self) -> Self {
        match self {
            Self::Mate(m) => Self::Mate(m + 1),
            Self::Centis(c) => Self::Centis(-c),
            Self::NegInfty => Self::Infty,
            Self::Infty => Self::NegInfty,
        }
    }
}

impl Neg for Eval {
    type Output = Eval;
    fn neg(self) -> Self {
        Eval {
            bound: match self.bound {
                Bound::Exact => Bound::Exact,
                Bound::Upper => Bound::Lower,
                Bound::Lower => Bound::Upper,
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
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.value {
            Value::Mate(m) => match other.value {
                Value::Mate(m2) => {
                    //Note that we have m2.cmp(m), since mate in 3 plys is better than mate in
                    //5 plys
                    if m % 2 == 1 && m2 % 2 == 1 {
                        Some(m2.cmp(&m))
                    } else if m % 2 == 1 && m2 % 2 == 0 {
                        Some(std::cmp::Ordering::Greater)
                    } else if m % 2 == 0 && m2 % 2 == 1 {
                        Some(std::cmp::Ordering::Less)
                    }
                    //If we get mated, a long time off is best!
                    else {
                        Some(m.cmp(&m2))
                    }
                }
                Value::Infty => Some(std::cmp::Ordering::Less),
                Value::NegInfty => Some(std::cmp::Ordering::Greater),
                Value::Centis(_) => {
                    if m % 2 == 1 {
                        Some(std::cmp::Ordering::Greater)
                    } else {
                        Some(std::cmp::Ordering::Less)
                    }
                }
            },
            Value::Infty => match other.value {
                Value::Infty => Some(std::cmp::Ordering::Equal),
                _ => Some(std::cmp::Ordering::Greater),
            },
            Value::NegInfty => match other.value {
                Value::NegInfty => Some(std::cmp::Ordering::Equal),
                _ => Some(std::cmp::Ordering::Less),
            },
            Value::Centis(c) => match other.value {
                Value::Mate(m) => {
                    if m % 2 == 1 {
                        Some(std::cmp::Ordering::Less)
                    } else {
                        Some(std::cmp::Ordering::Greater)
                    }
                }
                Value::Infty => Some(std::cmp::Ordering::Less),
                Value::NegInfty => Some(std::cmp::Ordering::Greater),
                Value::Centis(c2) => Some(c.cmp(&c2)),
            },
        }
    }
}

impl Add<i32> for Eval {
    type Output = Self;
    fn add(self, rhs: i32) -> Self::Output {
        match self.value {
            Value::Centis(c) => Eval {
                value: Value::Centis(rhs + c),
                bound: self.bound,
            },
            _ => self,
        }
    }
}

impl Sub<i32> for Eval {
    type Output = Self;
    fn sub(self, rhs: i32) -> Self::Output {
        match self.value {
            Value::Centis(c) => Eval {
                value: Value::Centis(c - rhs),
                bound: self.bound,
            },
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
    assert!(
        -Eval::MATE_NOW
            == Eval {
                bound: Bound::Exact,
                value: Value::Mate(1)
            }
    );
    assert!(
        -(-(-Eval::MATE_NOW))
            == Eval {
                bound: Bound::Exact,
                value: Value::Mate(3)
            }
    );
}
