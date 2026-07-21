use std::hint::black_box;

fn main() {
    let iterations: usize = std::env::var("ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50_000_000);

    let cases: &[(&str, &str)] = &[
        ("hello", "hello"),
        ("fred", "jane"),
        ("rfc2.txt", "rfc10.txt"),
        ("a10", "a2"),
        ("1.001", "1.002"),
        ("12345678901234", "12345678901235"),
        ("pic 5 something", "pic 6"),
        ("pic4   alpha", "pic 4 else"),
        (
            "the quick brown fox jumps over the lazy dog 1234567890",
            "the quick brown fox jumps over the lazy dog 1234567890",
        ),
        ("prefix_abcdef_12345_suffix", "prefix_abcdef_12346_suffix"),
        ("00123", "001234"),
        ("99999999999999999999", "99999999999999999998"),
        ("abcdefghijklmnopqrstuvwxyz", "abcdefghijklmnopqrstuvwxyA"),
        ("AlphabetSoup0123", "alphabetSoup0123"),
        ("AbCdEf123", "aBcDeF456"),
    ];

    // Warmup
    for (a, b) in cases {
        for _ in 0..1000 {
            black_box(fast_natord::compare(black_box(a), black_box(b)));
            black_box(fast_natord::compare_ignore_case(black_box(a), black_box(b)));
        }
    }

    let mut sum: u8 = 0;
    for i in 0..iterations {
        let (a, b) = cases[i % cases.len()];
        sum ^= black_box(fast_natord::compare(black_box(a), black_box(b)) as u8);
        sum ^= black_box(fast_natord::compare_ignore_case(black_box(a), black_box(b)) as u8);
    }
    eprintln!("sum={}", sum);
}
