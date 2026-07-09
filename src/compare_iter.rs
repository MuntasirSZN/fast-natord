use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

/// Compares two iterators of "characters" possibly containing "digits".
/// The natural ordering can be customized with the following parameters:
///
/// * `skip` returns true if the "character" does not affect the comparison,
///   other than splitting two consecutive digits.
/// * `cmp` compares two "characters", assuming that they are not "digits".
/// * `to_digit` converts a "character" into a "digit" if possible. The digit of zero is special.
pub fn compare_iter<T, L, R, Skip, Cmp, ToDigit>(
    left: L,
    right: R,
    mut skip: Skip,
    mut cmp: Cmp,
    mut to_digit: ToDigit,
) -> Ordering
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
    Skip: FnMut(&T) -> bool,
    Cmp: FnMut(&T, &T) -> Ordering,
    ToDigit: FnMut(&T) -> Option<isize>,
{
    let mut left = left;
    let mut right = right;
    let mut left_done = false;
    let mut right_done = false;

    let mut l;
    let mut r;

    macro_rules! read_left {
        () => {{
            l = if left_done {
                None
            } else {
                let n = left.next();
                if n.is_none() {
                    left_done = true;
                }
                n
            };
        }};
    }

    macro_rules! read_right {
        () => {{
            r = if right_done {
                None
            } else {
                let n = right.next();
                if n.is_none() {
                    right_done = true;
                }
                n
            };
        }};
    }

    macro_rules! return_unless_equal {
        ($ord:expr) => {
            match $ord {
                Equal => {}
                lastcmp => return lastcmp,
            }
        };
    }

    read_left!();
    read_right!();
    'nondigits: loop {
        while l.as_ref().is_some_and(&mut skip) {
            read_left!();
        }
        while r.as_ref().is_some_and(&mut skip) {
            read_right!();
        }

        match (l, r) {
            (Some(l_), Some(r_)) => match (to_digit(&l_), to_digit(&r_)) {
                (Some(ll_), Some(rr_)) => {
                    if ll_ == 0 || rr_ == 0 {
                        // left-aligned matching (`015` < `12`)
                        return_unless_equal!(ll_.cmp(&rr_));
                        'digits_left: loop {
                            read_left!();
                            read_right!();
                            let ll = l.as_ref().and_then(&mut to_digit);
                            let rr = r.as_ref().and_then(&mut to_digit);
                            match (ll, rr) {
                                (Some(ll_), Some(rr_)) => {
                                    return_unless_equal!(ll_.cmp(&rr_))
                                }
                                (Some(_), None) => return Greater,
                                (None, Some(_)) => return Less,
                                (None, None) => break 'digits_left,
                            }
                        }
                    } else {
                        // right-aligned matching (`15` < `123`)
                        let mut lastcmp = ll_.cmp(&rr_);
                        'digits_right: loop {
                            read_left!();
                            read_right!();
                            let ll = l.as_ref().and_then(&mut to_digit);
                            let rr = r.as_ref().and_then(&mut to_digit);
                            match (ll, rr) {
                                (Some(ll_), Some(rr_)) => {
                                    if lastcmp == Equal {
                                        lastcmp = ll_.cmp(&rr_);
                                    }
                                }
                                (Some(_), None) => return Greater,
                                (None, Some(_)) => return Less,
                                (None, None) => break 'digits_right,
                            }
                        }
                        return_unless_equal!(lastcmp);
                    }
                    continue 'nondigits;
                }
                (_, _) => return_unless_equal!(cmp(&l_, &r_)),
            },
            (Some(_), None) => return Greater,
            (None, Some(_)) => return Less,
            (None, None) => return Equal,
        }

        read_left!();
        read_right!();
    }
}
