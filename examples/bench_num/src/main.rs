use criterion::{black_box, criterion_group, criterion_main, Criterion};
use num::BigInt;

fn fibonacci_bigint(n: u64) -> BigInt {
    let mut a = BigInt::from(0);
    let mut b = BigInt::from(1);

    for _ in 0..n {
        let temp = &a + &b;
        a = b;
        b = temp;
    }

    a
}

fn bench_bigint_multiply(c: &mut Criterion) {
    let x = fibonacci_bigint(100);
    let y = fibonacci_bigint(100);

    c.bench_function("bigint_multiply_fib100", |b| {
        b.iter(|| black_box(&x) * black_box(&y))
    });
}

fn bench_bigint_fibonacci(c: &mut Criterion) {
    c.bench_function("bigint_fibonacci_50", |b| {
        b.iter(|| fibonacci_bigint(black_box(50)))
    });
}

criterion_group!(benches, bench_bigint_multiply, bench_bigint_fibonacci);
criterion_main!(benches);
