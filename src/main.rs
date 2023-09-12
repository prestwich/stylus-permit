// Only run this as a WASM if the export-abi feature is not set.
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

/// Initializes a custom, global allocator for Rust programs compiled to WASM.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use alloy_primitives::{fixed_bytes, FixedBytes};
use erc20permit::{DomainInfo, Erc20Details, Erc20Permit};
use stylus_sdk::stylus_proc::{entrypoint, external, sol_storage};

mod ecrecover;
mod erc20permit;

sol_storage! {
    #[entrypoint]
    pub struct MyErc20PermitContract {
        #[borrow]
        Erc20Permit<MyDomain, MyDetails> erc20;
    }
}

pub struct MyDomain;

impl DomainInfo for MyDomain {
    const NAME: Option<&'static str> = Some("my dumb token");

    const VERSION: Option<&'static str> = Some("1");

    const SALT: Option<FixedBytes<32>> = Some(fixed_bytes!(
        "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
    ));
}

pub struct MyDetails;

impl Erc20Details for MyDetails {
    const NAME: &'static str = "My Dumb Token";

    const SYMBOL: &'static str = "MDT";

    const DECIMALS: u8 = 18;
}

#[external]
#[inherit(Erc20Permit<MyDomain, MyDetails>)]
impl MyErc20PermitContract {}
