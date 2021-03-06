use byteorder::{BigEndian, ByteOrder};
use tiny_keccak::Keccak;
use parity_hash::H256;

use lib::*;
use super::{Signature, ValueType};
use super::util::Error;

#[derive(Clone)]
pub struct HashSignature {
    pub hash: u32,
    pub signature: Signature,
}

#[derive(Clone)]
pub struct NamedSignature {
	name: Cow<'static, str>,
	signature: Signature,
}

#[derive(Default)]
pub struct Table {
	// slice instead of hashmap since dispatch table is usually small (todo: maybe add variant with hash tables)
	pub inner: Cow<'static, [HashSignature]>,

	// anonymous signature/constructor
	pub fallback: Option<Signature>,
}

impl From<NamedSignature> for HashSignature {
	fn from(named: NamedSignature) -> HashSignature {
		let hash = named.hash();
		let signature = named.signature;

		HashSignature {
			hash: BigEndian::read_u32(&hash.as_ref()[0..4]),
			signature: signature
		}
	}
}

impl Table {
	pub fn new<T>(inner: T) -> Self
		where T: Into<Cow<'static, [HashSignature]>>
	{
		Table { inner: inner.into(), fallback: None }
	}

	pub fn with_fallback<T>(inner: T, fallback: Signature) -> Self
		where T: Into<Cow<'static, [HashSignature]>>
	{
		Table { inner: inner.into(), fallback: Some(fallback) }
	}

	pub fn push<S>(&mut self, signature: S)
		where S: Into<HashSignature>
	{
		self.inner.to_mut().push(signature.into())
	}

	pub fn dispatch<D>(&self, payload: &[u8], mut d: D) -> Result<Vec<u8>, Error>
		where D: FnMut(u32, Vec<ValueType>) -> Option<ValueType>
	{
		if payload.len() < 4 { return Err(Error::NoLengthForSignature); }
		let method_id = BigEndian::read_u32(&payload[0..4]);

		let hash_signature = self.hash_signature(method_id)?;

		let args = hash_signature.signature.decode_invoke(&payload[4..]);
		let result = d(method_id, args);

		Ok(hash_signature.signature.encode_result(result)?)
	}

	/// Fallback/constructor dispatch cannot return anything
	pub fn fallback_dispatch<D>(&self, payload: &[u8], mut d: D)
		-> Result<(), Error>
		where D: FnMut(Vec<ValueType>)
	{
		if let Some(ref fallback_signature) = self.fallback {
			d(fallback_signature.decode_invoke(payload));
			Ok(())
		} else {
			Err(Error::NoFallback)
		}
	}

	pub fn hash_signature(&self, method_id: u32) -> Result<&HashSignature, Error> {
		self.inner.iter().find(|x| x.hash == method_id).ok_or(Error::UnknownSignature)
	}

	pub fn call<D>(&self, hash: u32, args: &[ValueType], mut d: D)
		-> Result<Option<ValueType>, Error>
		where D: FnMut(Vec<u8>) -> Option<[u8; 32]>
	{
		let hash_signature = self.hash_signature(hash)?;
		let args_payload = hash_signature.signature.encode_invoke(args);
		let mut payload = Vec::with_capacity(args_payload.len() + 4);
		let mut encoded_signature = [0u8; 4];
		BigEndian::write_u32(&mut encoded_signature, hash);
		payload.extend_from_slice(&encoded_signature);
		payload.extend(args_payload);

		let result = d(payload);
		Ok(match result {
			Some(ref result_slice) => hash_signature.signature.decode_result(&result_slice[..])?,
			None => None,
		})
	}
}

impl NamedSignature {
	pub fn new<T>(name: T, signature: Signature) -> Self
		where T: Into<Cow<'static, str>>
	{
		NamedSignature {
			name: name.into(),
			signature: signature,
		}
	}

	pub fn name(&self) -> &str {
		self.name.as_ref()
	}

	pub fn signature(&self) -> &Signature {
		&self.signature
	}

	pub fn hash(&self) -> H256 {
		let mut signature_str = self.name.to_string();
		signature_str.push('(');
		for (i, p) in self.signature.params().iter().enumerate() {
			p.to_member(&mut signature_str);
			if i != self.signature.params().len()-1 { signature_str.push(','); }
		}
		signature_str.push(')');

		let mut keccak = Keccak::new_keccak256();
		let mut res = H256::zero();
		keccak.update(signature_str.as_bytes());
		keccak.finalize(res.as_mut());
		res
	}
}

impl HashSignature {
	pub fn new(hash: u32, signature: Signature) -> Self {
		HashSignature {
			hash: hash,
			signature: signature,
		}
	}

	pub fn hash(&self) -> u32 {
		self.hash
	}

	pub fn signature(&self) -> &Signature {
		&self.signature
	}
}

#[test]
fn match_signature() {

	use super::ParamType;

	let named = NamedSignature {
		name: Cow::Borrowed("baz"),
		signature: Signature::new_void(vec![ParamType::U32, ParamType::Bool]),
	};

	let hashed: HashSignature = named.into();

	assert_eq!(hashed.hash, 0xcdcd77c0);
}

#[test]
fn match_signature_2() {

	use super::ParamType;

	let named = NamedSignature {
		name: Cow::Borrowed("sam"),
		signature: Signature::new_void(vec![ParamType::Bytes, ParamType::Bool, ParamType::Array(ParamType::U256.into())]),
	};

	let hashed: HashSignature = named.into();

	assert_eq!(hashed.hash, 0xa5643bf2);
}

#[test]
fn table() {

	use super::ParamType;

	let mut table = Table::default();

	table.push(
		NamedSignature {
			name: Cow::Borrowed("baz"),
			signature: Signature::new_void(vec![ParamType::U32, ParamType::Bool]),
		}
	);

	table.push(
		NamedSignature {
			name: Cow::Borrowed("sam"),
			signature: Signature::new_void(vec![ParamType::Bytes, ParamType::Bool, ParamType::Array(ParamType::U256.into())]),
		}
	);

	table.dispatch(
		&[
			0xcd, 0xcd, 0x77, 0xc0,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x45,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01
		],
		|method_id, values| {
			assert_eq!(method_id, 0xcdcd77c0);
			assert_eq!(values[0], ValueType::U32(69));
			assert_eq!(values[1], ValueType::Bool(true));
			None
		}
	).expect("dispatch failed");

	table.dispatch(
		&[
			0xa5, 0x64, 0x3b, 0xf2,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x60,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa0,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
			0x64, 0x61, 0x76, 0x65, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
		],
		|method_id, values| {
			assert_eq!(method_id, 0xa5643bf2);
			assert_eq!(values[0], ValueType::Bytes(vec![100, 97, 118, 101]));
			assert_eq!(values[1], ValueType::Bool(true));
			assert_eq!(values[2], ValueType::Array(
				vec![
					ValueType::U256([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
					ValueType::U256([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02]),
					ValueType::U256([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03])
				]
			));
			None
		}
	).expect("dispatch failed");
}
