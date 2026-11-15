use super::*;

#[test]
fn jitter_is_clamped_to_the_widest_spread() {
    assert_eq!(Jitter::new(0.9), Jitter::new(0.5));
    assert_eq!(Jitter::new(-1.0), Jitter::new(-0.5));
    assert_eq!(Jitter::new(f64::NAN), Jitter::NONE);
}

#[test]
fn spread_is_clamped_and_bounds_jitter() {
    assert_eq!(Spread::new(2.0), Spread::MAX);
    assert_eq!(Spread::new(-0.1), Spread::NONE);
    assert_eq!(Spread::new(f64::NAN), Spread::NONE);
    assert_eq!(Spread::DEFAULT.bound(Jitter::new(0.5)), Jitter::new(0.2));
    assert_eq!(Spread::NONE.bound(Jitter::new(-0.5)), Jitter::NONE);
}

#[test]
fn samples_stay_within_the_spread() {
    for _ in 0..1_000 {
        let fraction = Spread::DEFAULT.sample().fraction();
        assert!((-0.2..=0.2).contains(&fraction), "{fraction}");
    }
    assert_eq!(Spread::NONE.sample(), Jitter::NONE);
}

#[test]
fn samples_vary() {
    let first = Spread::DEFAULT.sample();
    assert!((0..100).any(|_| Spread::DEFAULT.sample() != first));
}
