%builtins range_check

from starkware.cairo.common.secp256r1.ec import (
    EcPoint,
    ec_double
)
from starkware.cairo.common.cairo_secp.bigint import BigInt3

func main{range_check_ptr}() {
    let x = BigInt3(235, 522, 111);
    let y = BigInt3(1323, 15124, 796759);

    let point = EcPoint(x, y);

    let (r) = ec_double(point);

    assert r.x.d0 = 64413149096815403908768532;
    assert r.x.d1 = 28841630551789071202278393;
    assert r.x.d2 = 11527965423300397026710769;

    assert r.y.d0 = 6162628527473476058419904;
    assert r.y.d1 = 69076668518034904023852368;
    assert r.y.d2 = 10886445027049641070037760;

    return ();
}
