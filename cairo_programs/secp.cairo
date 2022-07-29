%builtins range_check
from starkware.cairo.common.cairo_secp.bigint import (
    nondet_bigint3,
    BigInt3,
)
from starkware.cairo.common.cairo_secp.field import (
    verify_zero,
    UnreducedBigInt3,
    reduce
)
func main{range_check_ptr: felt}():
    let zero: UnreducedBigInt3 = UnreducedBigInt3(0,0,0)
    verify_zero(zero)

    let x: UnreducedBigInt3 = UnreducedBigInt3(132181232131231239112312312313213083892150,10,10)

    let (y: BigInt3) = reduce(x)
    assert y = BigInt3(48537904510172037887998390,1708402383786350,10)

    let m: BigInt3 = nondet_bigint3()
    assert m = y

    let n: BigInt3 = reduce(UnreducedBigInt3(1321812083892150,11230,103321))
    assert n = BigInt3(1321812083892150,11230,103321)

    let p: BigInt3 = reduce(UnreducedBigInt3(0,0,0))
    assert p = BigInt3(0,0,0)

    let q: BigInt3 = reduce(UnreducedBigInt3(-10,0,0))
    assert q = BigInt3(77371252455336262886226981,77371252455336267181195263, 19342813113834066795298815)
    
    return()
end
