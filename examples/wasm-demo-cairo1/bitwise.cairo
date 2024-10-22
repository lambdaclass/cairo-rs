fn main() -> u128 {
    let a = 1234_u128;
    let b = 5678_u128;

    let c0 = a & b;
    let c1 = a ^ b;
    let c2 = a | b;

    let c3 = c0 + c1 + c2;

    c3
}
