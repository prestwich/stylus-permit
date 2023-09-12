use alloy_primitives::{address, Address, FixedBytes, U256};
use alloy_sol_types::{sol, sol_data, SolType};
use stylus_sdk::call::{self, Call};

const ECRECOVER: Address = address!("0000000000000000000000000000000000000001");

/// Invoke the ECRECOVER precompile.
pub fn ecrecover(
    hash: FixedBytes<32>,
    v: u8,
    r: U256,
    s: U256,
) -> Result<Address, stylus_sdk::call::Error> {
    let data = <sol! { (bytes32, uint8, uint256, uint256) }>::encode(&(*hash, v, r, s));

    call::static_call(Call::new(), ECRECOVER, &data)
        .map(|ret| sol_data::Address::decode_single(ret.as_slice(), false).unwrap())
}
