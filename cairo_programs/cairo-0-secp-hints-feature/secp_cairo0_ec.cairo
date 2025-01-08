%builtins range_check

// Tests:
// - cairo0_hints::COMPUTE_Q_MOD_PRIME
// - cairo0_hints::COMPUTE_IDS_HIGH_LOW
// - cairo0_hints::SECP_DOUBLE_ASSIGN_NEW_X
// - cairo0_hints::FAST_SECP_ADD_ASSIGN_NEW_Y

from starkware.cairo.common.secp256r1.ec import (
    EcPoint,
    compute_doubling_slope,
    compute_slope,
    ec_double,
    fast_ec_add,
    ec_mul_inner,
)
from starkware.cairo.common.cairo_secp.bigint3 import BigInt3

func main{range_check_ptr: felt}() {
    let x = BigInt3(1, 5, 10);
    let y = BigInt3(2, 4, 20);

    
    let point_a = EcPoint(x, y);

    let point_b = EcPoint(
        BigInt3(1, 5, 10),
        BigInt3(77371252455336262886226989, 77371252455336267181195259, 19342813113834066795298795),
    );

    // let (point_c) = ec_negate(EcPoint(BigInt3(156, 6545, 100010), BigInt3(1123, -1325, 910)));
    let point_c = EcPoint(
        BigInt3(156, 6545, 100010),
        BigInt3(77371252455336262886225868, 1324, 19342813113834066795297906),
    );

    // compute_doubling_slope
    let (slope_a) = compute_doubling_slope(point_b);
    assert slope_a = BigInt3(
        64839545681970757313529612, 5953360968438044038987377, 13253714962539897079325475
    );

    let (slope_b) = compute_doubling_slope(
        EcPoint(BigInt3(1231, 51235643, 100000), BigInt3(77371252455, 7737125245, 19342813113))
    );
    assert slope_b = BigInt3(
        61129622008745017597879703, 29315582959606925875642332, 13600923539144215962821694
    );

    // compute_slope
    let (slope_c) = compute_slope(point_a, point_c);
    assert slope_c = BigInt3(
        69736698275759322439409874, 45955733659898858347886847, 18034242868575077772302310
    );

    let (slope_d) = compute_slope(point_c, point_b);
    assert slope_d = BigInt3(
        66872739393348882319301304, 44057296979296181456999622, 6628179500048909995474229
    );

    // ec_double
    let (point_d) = ec_double(point_a);
    assert point_d = EcPoint(
        BigInt3(62951442591564288805558802, 32562108923955565608466346, 18605500881547971871596634),
        BigInt3(32147810383256899543807670, 5175857156528420748725791, 6618806236944685895112117),
    );

    let (point_e) = ec_double(
        EcPoint(BigInt3(156, 6545, 100010), BigInt3(5336262886225868, 1324, 113834066795297906))
    );
    assert point_e = EcPoint(
        BigInt3(47503316700827173496989353, 17218105161352860131668522, 527908748911931938599018),
        BigInt3(50964737623371959432443726, 60451660835701602854498663, 5043009036652075489876599),
    );

    // fast_ec_add
    let (point_f) = fast_ec_add(point_a, point_e);
    assert point_f = EcPoint(
        BigInt3(29666922781464823323928071, 37719311829566792810003084, 9541551049028573381125035),
        BigInt3(12938160206947174373897851, 22954464827120147659997987, 2690642098017756659925259),
    );

    let (point_g) = fast_ec_add(
        EcPoint(BigInt3(89712, 56, 7348489324), BigInt3(980126, 10, 8793)),
        EcPoint(BigInt3(16451, 5967, 2171381), BigInt3(12364564, 123654, 193)),
    );
    assert point_g = EcPoint(
        BigInt3(14771767859485410664249539, 62406103981610765545970487, 8912032684309792565082157),
        BigInt3(25591125893919304137822981, 54543895003572926651874352, 18968003584818937876851951),
    );

    // ec_mul_inner
    let (pow2, res) = ec_mul_inner(
        EcPoint(
            BigInt3(65162296, 359657, 04862662171381), BigInt3(5166641367474701, 63029418, 793)
        ),
        123,
        298,
    );
    assert pow2 = EcPoint(
        BigInt3(73356165220405599685396595, 44054642183803477920871071, 5138516367480965019117743),
        BigInt3(40256732918865941543909206, 68107624737772931608959283, 1842118797516663063623771),
    );
    return ();
}
