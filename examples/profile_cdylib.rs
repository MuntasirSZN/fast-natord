fn main() {
    let iterations: usize = std::env::var("ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50_000_000);

    let cases: &[(&str, &str)] = &[
        ("hello", "hello"),
        ("rfc2.txt", "rfc10.txt"),
        ("a10", "a2"),
        ("00123", "001234"),
        ("pic 5 something", "pic 6"),
        (
            "the quick brown fox jumps over the lazy dog 1234567890",
            "the quick brown fox jumps over the lazy dog 1234567890",
        ),
        ("prefix_abcdef_12345_suffix", "prefix_abcdef_12346_suffix"),
        ("99999999999999999999", "99999999999999999998"),
    ];

    for (a, b) in cases {
        for _ in 0..1000 {
            std::hint::black_box(fast_natord::compare(
                std::hint::black_box(a),
                std::hint::black_box(b),
            ));
        }
    }

    let mut sum: u8 = 0;
    for i in 0..iterations {
        let (a, b) = cases[i % cases.len()];
        sum ^= std::hint::black_box(fast_natord::compare(
            std::hint::black_box(a),
            std::hint::black_box(b),
        )) as u8;
    }
    eprintln!("sum={}", sum);
}
