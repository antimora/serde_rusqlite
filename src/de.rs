extern crate rusqlite;
extern crate serde;

use super::{Error, Result};
use self::rusqlite::types::Value;
use self::serde::de;
use self::serde::de::IntoDeserializer;
use std::{f32, f64};

macro_rules! forward_to_row_value_deserializer {
	($($fun:ident)*) => {
		$(
			fn $fun<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
				self.row_value().$fun(visitor)
			}
		)*
	}
}

/// Deserializer for `rusqlite::Row`
///
/// You shouldn't use it directly, but via the crate's `from_row()` function. Check the crate documentation for example.
pub struct RowDeserializer<'de> {
	row: &'de rusqlite::Row<'de, 'de>,
	columns: Option<&'de [&'de str]>,
}

impl<'de> RowDeserializer<'de> {
	pub fn from_row_with_columns(row: &'de rusqlite::Row, columns: &'de [&'de str]) -> Self {
		Self { row, columns: Some(columns) }
	}

	pub fn from_row(row: &'de rusqlite::Row) -> Self {
		Self { row, columns: None }
	}

	fn row_value(&self) -> RowValue<'de, usize> {
		RowValue { row: self.row, idx: 0 }
	}
}

impl<'de> de::Deserializer<'de> for RowDeserializer<'de> {
	type Error = Error;

	fn deserialize_map<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_map(RowMapAccess { idx: 0, de: self })
	}

	fn deserialize_struct<V: de::Visitor<'de>>(mut self, _name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value> {
		self.columns = Some(fields);
		self.deserialize_map(visitor)
	}

	fn deserialize_newtype_struct<V: de::Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value> {
		visitor.visit_newtype_struct(self.row_value())
	}

	fn deserialize_tuple<V: de::Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
		visitor.visit_seq(RowSeqAccess { idx: 0, de: self })
	}

	fn deserialize_unit_struct<V: de::Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value> {
		self.row_value().deserialize_unit_struct(name, visitor)
	}

	fn deserialize_enum<V: de::Visitor<'de>>(self, name: &'static str, variants: &'static [&'static str], visitor: V) -> Result<V::Value> {
		self.row_value().deserialize_enum(name, variants, visitor)
	}

	forward_to_row_value_deserializer! {
		deserialize_bool
		deserialize_f32
		deserialize_f64
		deserialize_option
		deserialize_unit
		deserialize_any
		deserialize_byte_buf
	}

	forward_to_deserialize_any! {
		i8 i16 i32 i64 u8 u16 u32 u64 char str string bytes
		seq tuple_struct identifier ignored_any
	}
}

struct RowValue<'row, RI> {
	idx: RI,
	row: &'row rusqlite::Row<'row, 'row>,
}

impl<'de, RI: rusqlite::RowIndex + Copy> RowValue<'de, RI> {
	fn value<T: rusqlite::types::FromSql>(&self) -> Result<T> {
		self.row.get_checked(self.idx).map_err(Error::from)
	}

	fn deserialize_any_helper<V: de::Visitor<'de>>(self, visitor: V, value: Value) -> Result<V::Value> {
		match value {
			Value::Null => visitor.visit_none(),
			Value::Integer(val) => visitor.visit_i64(val),
			Value::Real(val) => visitor.visit_f64(val),
			Value::Text(val) => visitor.visit_string(val),
			Value::Blob(val) => visitor.visit_seq(val.into_deserializer()),
		}
	}
}

impl<'de, RI: rusqlite::RowIndex + Copy> de::Deserializer<'de> for RowValue<'de, RI> {
	type Error = Error;

	fn deserialize_bool<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		match self.value()? {
			Value::Integer(val) => visitor.visit_bool(val != 0),
			Value::Real(val) => visitor.visit_bool(val != 0.),
			val => self.deserialize_any_helper(visitor, val),
		}
	}

	fn deserialize_f32<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		match self.value()? {
			Value::Null => visitor.visit_f32(f32::NAN),
			val => self.deserialize_any_helper(visitor, val),
		}
	}

	fn deserialize_f64<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		match self.value()? {
			Value::Null => visitor.visit_f64(f64::NAN),
			val => self.deserialize_any_helper(visitor, val),
		}
	}

	fn deserialize_byte_buf<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		visitor.visit_byte_buf(self.value()?)
	}

	fn deserialize_option<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		match self.value()? {
			Value::Null => visitor.visit_none(),
			_ => visitor.visit_some(self),
		}
	}

	fn deserialize_unit<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		match self.value()? {
			Value::Null => visitor.visit_unit(),
			val => self.deserialize_any_helper(visitor, val),
		}
	}

	fn deserialize_unit_struct<V: de::Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value> {
		match self.value()? {
			Value::Text(ref val) if val == name => visitor.visit_unit(),
			val => self.deserialize_any_helper(visitor, val),
		}
	}

	fn deserialize_enum<V: de::Visitor<'de>>(self, _name: &'static str, _variants: &'static [&'static str], visitor: V) -> Result<V::Value> {
		visitor.visit_enum(EnumAccess(self.value()?))
	}

	fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		let val = self.value()?;
		self.deserialize_any_helper(visitor, val)
	}

	forward_to_deserialize_any! {
		i8 i16 i32 i64 u8 u16 u32 u64 char str string bytes
		newtype_struct seq tuple
		tuple_struct map struct identifier ignored_any
	}
}

struct RowMapAccess<'de> {
	idx: usize,
	de: RowDeserializer<'de>,
}

impl<'de> de::MapAccess<'de> for RowMapAccess<'de> {
	type Error = Error;

	fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
		match self.de.columns {
			Some(columns) => {
				if self.idx as usize >= columns.len() {
					Ok(None)
				} else {
					seed.deserialize(columns[self.idx as usize].into_deserializer()).map(Some)
				}
			},
			None => Ok(None)
		}
	}

	fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
		if let Some(columns) = self.de.columns {
			let out = seed.deserialize(RowValue { idx: columns[self.idx as usize], row: self.de.row });
			self.idx += 1;
			out
		} else {
			unreachable!()
		}
	}
}

struct RowSeqAccess<'de> {
	idx: usize,
	de: RowDeserializer<'de>,
}

impl<'de> de::SeqAccess<'de> for RowSeqAccess<'de> {
	type Error = Error;

	fn next_element_seed<T: de::DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
		let out = seed.deserialize(RowValue { idx: self.idx, row: self.de.row }).map(Some);
		self.idx += 1;
		out
	}
}

struct EnumAccess(String);

impl<'de> de::EnumAccess<'de> for EnumAccess {
	type Error = Error;
	type Variant = VariantAccess;

	fn variant_seed<V: de::DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self::Variant)> {
		seed.deserialize(self.0.into_deserializer()).map(|v| (v, VariantAccess))
	}
}

struct VariantAccess;

impl<'de> de::VariantAccess<'de> for VariantAccess {
	type Error = Error;

	fn unit_variant(self) -> Result<()> {
		Ok(())
	}

	fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value> { Err(Error::de_unsupported("newtype_variant").into()) }
	fn tuple_variant<V: de::Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value> { Err(Error::de_unsupported("tuple_variant")) }
	fn struct_variant<V: de::Visitor<'de>>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value> { Err(Error::de_unsupported("struct_variant")) }
}
