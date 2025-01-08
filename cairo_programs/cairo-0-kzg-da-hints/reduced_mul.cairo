%builtins range_check

from starkware.starknet.core.os.data_availability.bls_field import reduced_mul, BigInt3

func main{range_check_ptr: felt}() {
    let x = BigInt3(0, 0, 0);
    let y = BigInt3(1, 1, 1);

    let res = reduced_mul(x, y);

    assert res = BigInt3(0, 0, 0);

    let x = BigInt3(100, 99, 98);
    let y = BigInt3(10, 9, 8);

    let res = reduced_mul(x, y);

    assert res = BigInt3(
        49091481911800146991175221, 43711329369885800715738617, 405132241597509509195407
    );

    let x = BigInt3(47503316700827173496989353, 17218105161352860131668522, 527908748911931938599018);
    let y = BigInt3(50964737623371959432443726, 60451660835701602854498663, 5043009036652075489876599);

    let res = reduced_mul(x, y);
    assert res = BigInt3(43476011663489831917914902, 15057238271740518603165849, 1923992965848504555868221);

    return ();
}
