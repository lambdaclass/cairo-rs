%builtins range_check

from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.cairo_secp.bigint3 import BigInt3
from starkware.cairo.common.cairo_secp.ec_point import EcPoint
from starkware.cairo.common.secp256r1.ec import (
    try_get_point_from_x
)
from starkware.cairo.common.uint256 import Uint256


func main{range_check_ptr: felt}() {
    let zero = BigInt3(
        0, 0, 0
    );
    let result: EcPoint* = alloc();
    let (is_on_curve) = try_get_point_from_x(zero, 0, result);
    assert is_on_curve = 1;

    let x = BigInt3(512,2412,133);
    let result: EcPoint* = alloc();
    let (is_on_curve) = try_get_point_from_x(x, 1, result);
    assert is_on_curve = 1;

    let x = BigInt3(64,0,6546);

    let result: EcPoint* = alloc();
    let (is_on_curve) = try_get_point_from_x(x, 1, result);
    assert is_on_curve = 0;
    return ();
}
