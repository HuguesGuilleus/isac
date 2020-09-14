use std::cmp::Ordering;

pub type Couple<A, B> = (Option<A>, Option<B>);

pub fn linkvec<A, B, SortA, SortB, E>(
    mut a: Vec<A>,
    sort_a: SortA,
    mut b: Vec<B>,
    sort_b: SortB,
    eq: E,
) -> Vec<Couple<A, B>>
where
    SortA: Fn(&A, &A) -> Option<Ordering>,
    SortB: Fn(&B, &B) -> Option<Ordering>,
    E: Fn(&A, &B) -> Ordering,
{
    a.sort_by(|a, b| sort_a(a, b).unwrap_or(Ordering::Equal).reverse());
    b.sort_by(|a, b| sort_b(a, b).unwrap_or(Ordering::Equal).reverse());

    let mut all: Vec<Couple<A, B>> = Vec::with_capacity(a.len() + b.len());

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
#[test]
fn test_linkvec() {
    assert_eq!(
        linkvec(
            vec![4, 6, 7, 8, 1, 2, 3],
            |a, b| a.partial_cmp(b),
            vec![4, 8, 10, 12, 13, 16, 17],
            |a, b| a.partial_cmp(b),
            |a: &isize, b: &isize| -> std::cmp::Ordering { (a * 2).partial_cmp(&b).unwrap() },
        ),
        vec![
            (Some(1), None),
            (Some(2), Some(4)),
            (Some(3), None),
            (Some(4), Some(8)),
            (None, Some(10)),
            (Some(6), Some(12)),
            (None, Some(13)),
            (Some(7), None),
            (Some(8), Some(16)),
            (None, Some(17)),
        ],
    );
}
