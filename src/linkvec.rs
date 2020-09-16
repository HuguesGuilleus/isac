use std::cmp::{Ordering, PartialOrd};

pub type Couple<A> = (Option<A>, Option<A>);

pub fn linkvec<A: PartialOrd>(mut a: Vec<A>, mut b: Vec<A>) -> Vec<Couple<A>> {
    a.sort_by(|a, b| eq(a, b).reverse());
    b.sort_by(|a, b| eq(a, b).reverse());

    let mut all: Vec<Couple<A>> = Vec::with_capacity(a.len() + b.len());

    let mut i = a.pop();
    let mut j = b.pop();
    while i.is_some() && j.is_some() {
        match eq(i.as_ref().unwrap(), j.as_ref().unwrap()) {
            std::cmp::Ordering::Equal => {
                all.push((i, j));
                i = a.pop();
                j = b.pop();
            }
            std::cmp::Ordering::Less => {
                all.push((i, None));
                i = a.pop();
            }
            std::cmp::Ordering::Greater => {
                all.push((None, j));
                j = b.pop();
            }
        }
    }
    while i.is_some() {
        all.push((i, None));
        i = a.pop();
    }
    while j.is_some() {
        all.push((None, j));
        j = b.pop();
    }

    all
}
fn eq<A: PartialOrd>(a: &A, b: &A) -> std::cmp::Ordering {
    a.partial_cmp(b).unwrap_or(Ordering::Equal)
}
#[test]
fn test_linkvec() {
    assert_eq!(
        linkvec(vec![1, 2, 3, 4, 6], vec![2, 4, 5, 6, 7]),
        vec![
            (Some(1), None),
            (Some(2), Some(2)),
            (Some(3), None),
            (Some(4), Some(4)),
            (None, Some(5)),
            (Some(6), Some(6)),
            (None, Some(7)),
        ],
    );
}
