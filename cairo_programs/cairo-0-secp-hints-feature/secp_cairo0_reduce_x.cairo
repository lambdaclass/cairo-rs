%builtins range_check

from starkware.cairo.common.cairo_secp.bigint import BigInt3, nondet_bigint3, UnreducedBigInt3

func reduce_x{range_check_ptr}(x: UnreducedBigInt3) -> (res: BigInt3) {
    %{ 
        from starkware.cairo.common.cairo_secp.secp256r1_utils import SECP256R1_P
        from starkware.cairo.common.cairo_secp.secp_utils import pack
        value = pack(ids.x, PRIME) % SECP256R1_P 
    %}
    let (res) = nondet_bigint3();
    return (res=res);
}

func test_reduce_x{range_check_ptr: felt}() {
     let x = UnreducedBigInt3(0, 0, 0);
     let (reduce_a) = reduce_x(x);
     assert reduce_a = BigInt3(
         0, 0, 0
     );
 
     let y = UnreducedBigInt3(12354, 745634534, 81298789312879123);
     let (reduce_b) = reduce_x(y);
     assert reduce_b = BigInt3(
         12354, 745634534, 81298789312879123
     );
 
     let z = UnreducedBigInt3(12354812987893128791212331231233, 7453123123123123312634534, 8129224990312325879);
     let (reduce_c) = reduce_x(z);
     assert reduce_c = BigInt3(
         16653320122975184709085185, 7453123123123123312794216, 8129224990312325879
     );
    return ();
}

func main{range_check_ptr: felt}() {
    test_reduce_x();
    return ();
}
