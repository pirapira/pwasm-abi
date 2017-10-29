
mod contract {
	use pwasm_abi_derive::eth_abi;
	use parity_hash::Address;
	use bigint::U256;
	use call;

	#[cfg(not(test))]
	use alloc::borrow::Cow;
	#[cfg(test)]
	use std::borrow::Cow;

	#[allow(non_snake_case)]
	#[eth_abi(Endpoint, Client)]
	pub trait TokenContract {
		fn ctor(&mut self, total_supply: U256);
		fn balanceOf(&mut self, _owner: Address) -> U256;
		fn transfer(&mut self, _to: Address, _amount: U256) -> bool;
		fn totalSupply(&mut self) -> U256;
	}
}
