%builtins range_check

from starkware.cairo.common.secp256r1.ec import (
    EcPoint,
    ec_mul_by_uint256
)
from starkware.cairo.common.uint256 import Uint256
from starkware.cairo.common.cairo_secp.bigint import BigInt3

func main{range_check_ptr: felt}() {
    let x = BigInt3(235, 522, 111);
    let y = BigInt3(1323, 15124, 796759);

    let point = EcPoint(x, y);

    let scalar = Uint256(
        143186476941636880901214103594843510573, 124026708105846590725274683684370988502
    );
    let (res) = ec_mul_by_uint256(point, scalar);

    assert res = EcPoint(
        BigInt3(31454759005629465428788733, 35370111304581841775514461, 13535495107675380502530193),
        BigInt3(18078210390106977421552565, 53503834862379828768870254, 3887397808398301655656699),
    );
    return ();
}
