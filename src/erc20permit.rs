use std::marker::PhantomData;

use alloy_primitives::{Address, FixedBytes, U256};
use alloy_sol_types::{sol, Eip712Domain, SolError, SolStruct};
use stylus_sdk::{
    block::{self, chainid},
    contract, msg,
    stylus_proc::{external, sol_storage},
};

/// Domain info for EIP-712
pub trait DomainInfo {
    const NAME: Option<&'static str>;
    const VERSION: Option<&'static str>;
    const SALT: Option<FixedBytes<32>>;
}

/// Erc20 details.
pub trait Erc20Details {
    const NAME: &'static str;
    const SYMBOL: &'static str;
    const DECIMALS: u8;
}

sol_storage! {
    pub struct Erc20Permit<T, U> {
        mapping (address => uint256) balances;
        uint256 total_supply;
        mapping (address => mapping(address => uint256)) allowances;

        mapping (address => uint256) nonces;

        PhantomData<T> domain;
        PhantomData<U> details;
    }
}

sol! {
    struct Permit {
        address owner;
        address spender;
        uint256 value;
        uint256 nonce;
        uint256 deadline;
    }

    contract Erc20 {
        #[derive(Default)]
        error PermitExpired();
        #[derive(Default)]
        error InvalidPermit();
        #[derive(Default)]
        error InsufficientBalance();
        #[derive(Default)]
        error InsufficientAllowance();

        event Transfer(address indexed from, address indexed to, uint256 amount);

        event Approval(address indexed owner, address indexed spender, uint256 amount);
    }
}

use Erc20::Erc20Errors;

use crate::ecrecover::ecrecover;
type Erc20Result<T> = Result<T, Erc20Errors>;

impl Erc20Errors {
    fn encode(&self) -> Vec<u8> {
        match self {
            Erc20Errors::PermitExpired(e) => e.encode(),
            Erc20Errors::InvalidPermit(e) => e.encode(),
            Erc20Errors::InsufficientBalance(e) => e.encode(),
            Erc20Errors::InsufficientAllowance(e) => e.encode(),
        }
    }
}

#[external]
impl<T, U> Erc20Permit<T, U>
where
    T: DomainInfo,
    U: Erc20Details,
{
    pub fn name() -> Result<String, Vec<u8>> {
        Ok(U::NAME.to_owned())
    }

    pub fn symbol() -> Result<String, Vec<u8>> {
        Ok(U::SYMBOL.to_owned())
    }

    pub fn decimals() -> Result<u8, Vec<u8>> {
        Ok(U::DECIMALS)
    }

    pub fn total_supply(&self) -> Result<U256, Vec<u8>> {
        Ok(self._total_supply())
    }

    pub fn balance_of(&self, owner: Address) -> Result<U256, Vec<u8>> {
        Ok(self._balance_of(owner))
    }

    pub fn transfer(&mut self, to: Address, amount: U256) -> Result<bool, Vec<u8>> {
        self._transfer(to, amount).map_err(|e| e.encode())
    }

    pub fn allowance(&self, owner: Address, spender: Address) -> Result<U256, Vec<u8>> {
        Ok(self._allowance(owner, spender))
    }

    pub fn approve(&mut self, spender: Address, amount: U256) -> Result<bool, Vec<u8>> {
        self._approve(spender, amount).map_err(|e| e.encode())
    }

    pub fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Result<bool, Vec<u8>> {
        self._transfer_from(from, to, amount)
            .map_err(|e| e.encode())
    }

    pub fn permit(
        &mut self,
        owner: Address,
        spender: Address,
        value: U256,
        deadline: U256,
        v: u8,
        r: U256,
        s: U256,
    ) -> Result<(), Vec<u8>> {
        self._permit(owner, spender, value, deadline, v, r, s)
            .map_err(|e| e.encode())
    }

    pub fn transfer_with_permit(
        &mut self,
        to: Address,
        amount: U256,

        owner: Address,
        spender: Address,
        value: U256,
        deadline: U256,
        v: u8,
        r: U256,
        s: U256,
    ) -> Result<bool, Vec<u8>> {
        self._transfer_with_permit(to, amount, owner, spender, value, deadline, v, r, s)
            .map_err(|e| e.encode())
    }
}

impl<T, U> Erc20Permit<T, U>
where
    T: DomainInfo,
    U: Erc20Details,
{
    pub fn _mint(&mut self, to: Address, amount: U256) -> Erc20Result<()> {
        let total = self.total_supply.get();

        self.saturating_credit(to, amount)?;
        self.total_supply.set(total + amount);

        Ok(())
    }

    pub fn _burn(&mut self, from: Address, amount: U256) -> Erc20Result<()> {
        let total = self.total_supply.get();

        let burned = self.saturating_debit(from, amount)?;
        self.total_supply.set(total - burned);

        Ok(())
    }

    fn get_domain(&self) -> Eip712Domain {
        Eip712Domain {
            name: T::NAME.map(std::borrow::Cow::Borrowed),
            version: T::VERSION.map(std::borrow::Cow::Borrowed),
            chain_id: Some(U256::from(chainid())),
            verifying_contract: Some(contract::address()),
            salt: T::SALT,
        }
    }

    /// Debits an account with the given amount, saturating the balance, and
    /// returning the amount actually debited.
    fn saturating_debit(&mut self, addr: Address, amount: U256) -> Erc20Result<U256> {
        let mut balance = self.balances.setter(addr);

        let new_bal = balance.get().saturating_sub(amount);
        let burned = balance.get() - new_bal;

        balance.set(new_bal);

        Ok(burned)
    }

    /// Debits an account with the given amount, returning an error if the
    /// balance is insufficient.
    fn debit(&mut self, addr: Address, amount: U256) -> Erc20Result<()> {
        let mut balance = self.balances.setter(addr);

        let bal = balance.get();
        if bal < amount {
            return Err(Erc20::Erc20Errors::InsufficientBalance(Default::default()));
        }
        balance.set(bal - amount);
        Ok(())
    }

    /// Credits an account with the given amount, saturating the balance, and
    /// returning the amount actually credited.
    fn saturating_credit(&mut self, addr: Address, amount: U256) -> Erc20Result<U256> {
        let mut balance = self.balances.setter(addr);

        let new_bal = balance.get().saturating_add(amount);
        let minted = new_bal - balance.get();
        balance.set(new_bal);

        Ok(minted)
    }

    /// Credits an account with the given amount.
    fn credit(&mut self, addr: Address, amount: U256) -> Erc20Result<()> {
        let mut balance = self.balances.setter(addr);

        let bal = balance.get();
        balance.set(bal + amount);
        Ok(())
    }

    fn move_tokens(&mut self, from: Address, to: Address, amount: U256) -> Erc20Result<()> {
        self.debit(from, amount)?;
        self.credit(to, amount)?;
        Ok(())
    }

    fn set_approval(&mut self, owner: Address, spender: Address, amount: U256) -> Erc20Result<()> {
        self.allowances.setter(owner).setter(spender).set(amount);
        Ok(())
    }

    fn increment_nonce(&mut self, owner: Address) -> Erc20Result<()> {
        let mut nonce = self.nonces.setter(owner);
        let next = nonce.get();
        nonce.set(next + U256::from(1));
        Ok(())
    }

    fn _total_supply(&self) -> U256 {
        self.total_supply.get()
    }

    fn _balance_of(&self, owner: Address) -> U256 {
        self.balances.get(owner)
    }

    fn _transfer(&mut self, to: Address, amount: U256) -> Erc20Result<bool> {
        self.move_tokens(msg::sender(), to, amount)?;
        Ok(true)
    }

    fn _allowance(&self, owner: Address, spender: Address) -> U256 {
        self.allowances.get(owner).get(spender)
    }

    fn _approve(&mut self, spender: Address, amount: U256) -> Erc20Result<bool> {
        self.set_approval(msg::sender(), spender, amount)?;
        Ok(true)
    }

    fn _transfer_from(&mut self, from: Address, to: Address, amount: U256) -> Erc20Result<bool> {
        let spender = msg::sender();
        let allowance = self._allowance(from, spender);

        if allowance < amount {
            return Err(Erc20::Erc20Errors::InsufficientAllowance(Default::default()));
        }
        self.set_approval(from, spender, allowance - amount)?;
        self.move_tokens(from, to, amount)?;

        Ok(true)
    }

    fn _permit(
        &mut self,
        owner: Address,
        spender: Address,
        value: U256,
        deadline: U256,
        v: u8,
        r: U256,
        s: U256,
    ) -> Erc20Result<()> {
        if owner == Address::ZERO {
            return Err(Erc20::Erc20Errors::InvalidPermit(Default::default()));
        }
        if U256::from(block::timestamp()) > deadline {
            return Err(Erc20::Erc20Errors::PermitExpired(Default::default()));
        }

        // Compute Permit signing hash
        let permit = Permit {
            owner,
            spender,
            value,
            nonce: self.nonces.get(owner),
            deadline,
        };
        let domain = self.get_domain();
        let permit_hash = permit.eip712_signing_hash(&domain);

        let recovered = ecrecover(permit_hash, v, r, s)
            .map_err(|_| Erc20Errors::InvalidPermit(Default::default()))?;

        if recovered != owner {
            return Err(Erc20::Erc20Errors::InvalidPermit(Default::default()));
        }

        self.set_approval(owner, spender, value)?;
        self.increment_nonce(owner)?;

        Ok(())
    }

    fn _transfer_with_permit(
        &mut self,
        to: Address,
        amount: U256,

        owner: Address,
        spender: Address,
        value: U256,
        deadline: U256,
        v: u8,
        r: U256,
        s: U256,
    ) -> Erc20Result<bool> {
        self._permit(owner, spender, value, deadline, v, r, s)?;
        self._transfer_from(owner, to, amount)
    }
}
