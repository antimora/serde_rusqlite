//! # Serde Rusqlite
//!
//! This crate provides convenience functions to bridge serde and rusqlite. With their help
//! you can "deserialize" rusqlite `Row`'s into serde `Deserialize` types and "serialize" types
//! implementing `Serialize` into bound query arguments (positional or named) that rusqlite expects.
//!
//! Serialization of named bound arguments is only supported from `struct`s and `map`s because other
//! serde types lack column name information. Likewise, serialization of positional bound arguments
//! is only supported from `tuple`s, `sequence`s and primitive non-iterable types. In the latter case
//! the result will be single-element vector. Each serialized field or element must implement
//! `rusqlite::types::ToSql`.
//!
//! For deserialization you can use two families of functions: `from_*()` and `from_*_with_columns()`.
//! The most used one is the former. The latter allows you to specify column names for types that need
//! them, but don't supply them. This includes different `Map` types like `HashMap`. Specifying columns
//! for deserialization into e.g. `struct` doesn't have any effect as the field list of the struct itself
//! will be used in any case.
//!
//! SQLite only supports 5 types: `NULL` (`None`), `INTEGER` (`i64`), `REAL` (`f64`), `TEXT` (`String`)
//! and `BLOB` (`Vec<u8>`). Corresponding rust types are inside brackets.
//!
//! Some types employ non-trivial handling, these are described below:
//!
//! * Serialization of `u64` will fail if it can't be represented by `i64` due to sqlite limitations.
//! * Simple `enum`s will be serialized as strings so:
//!
//!   ```
//!   enum Gender {
//!      M,
//!      F,
//!   }
//!   ```
//!
//!   will have two possible `TEXT` options in the database "M" and "F". Deserialization into `enum`
//!   from `TEXT` is also supported.
//! * `bool`s are serialized as `INTEGER`s 0 or 1, can be deserialized from `INTEGER` and `REAL` where
//!   0 and 0.0 are `false`, anything else is `true`.
//! * `f64` and `f32` values of `NaN` are serialized as `NULL`s. When deserializing such value `Option<f64>`
//!   will have value of `None` and `f64` will have value of `NaN`. The same applies to `f32`.
//! * `Bytes`, `ByteBuf` from `serde_bytes` are supported as optimized way of handling `BLOB`s.
//! * `unit` serializes to `NULL`.
//! * Only `sequence`s of `u8` are serialized and deserialized, `BLOB` database type is used. It's
//!   more optimal though to use `Bytes` and `ByteBuf` from `serde_bytes` for such fields.
//! * `unit_struct` serializes to `struct` name as `TEXT`, when deserializing the check is made to ensure
//!   that `struct` name coincides with the string in the database.
//!
//! # Examples
//! ```
//! extern crate rusqlite;
//! #[macro_use]
//! extern crate serde_derive;
//! extern crate serde_rusqlite;
//!
//! use serde_rusqlite::*;
//! use rusqlite::NO_PARAMS;
//!
//! #[derive(Serialize, Deserialize, Debug, PartialEq)]
//! struct Example {
//!    id: i64,
//!    name: String,
//! }
//!
//! fn main() {
//!    let connection = rusqlite::Connection::open_in_memory().unwrap();
//!    connection.execute_batch("CREATE TABLE example (id INT, name TEXT)").unwrap();
//!
//!    // using structure to generate named bound query arguments
//!    let row1 = Example{ id: 1, name: "first name".into() };
//!    connection.execute_named("INSERT INTO example (id, name) VALUES (:id, :name)", &to_params_named(&row1).unwrap().to_slice()).unwrap();
//!
//!    // using tuple to generate positional bound query arguments
//!    let row2 = (2, "second name");
//!    connection.execute("INSERT INTO example (id, name) VALUES (?, ?)", &to_params(&row2).unwrap().to_slice()).unwrap();
//!
//!    // deserializing using query() and from_rows()
//!    let mut statement = connection.prepare("SELECT * FROM example").unwrap();
//!    let mut res = from_rows::<Example>(statement.query(NO_PARAMS).unwrap());
//!    assert_eq!(res.next().unwrap(), row1);
//!    assert_eq!(res.next().unwrap(), Example{ id: 2, name: "second name".into() });
//!
//!    // deserializing using query_map() and from_row()
//!    let mut statement = connection.prepare("SELECT * FROM example").unwrap();
//!    let mut rows = statement.query_map(NO_PARAMS, from_row::<Example>).unwrap();
//!    assert_eq!(rows.next().unwrap().unwrap().unwrap(), row1);
//!    assert_eq!(rows.next().unwrap().unwrap().unwrap(), Example{ id: 2, name: "second name".into() });
//!
//!    // deserializing using query() and from_rows_ref()
//!    let mut statement = connection.prepare("SELECT * FROM example").unwrap();
//!    let mut rows = statement.query(NO_PARAMS).unwrap();
//!    {
//!       // only first record is deserialized here
//!       let mut res = from_rows_ref::<Example>(&mut rows);
//!       assert_eq!(res.next().unwrap(), row1);
//!    }
//!    // the second record is deserialized using the original Rows iterator
//!    assert_eq!(from_row::<Example>(&rows.next().unwrap().unwrap()).unwrap(), Example{ id: 2, name: "second name".into() });
//!
//!    // deserializing using query() and from_rows_with_columns()
//!    let mut statement = connection.prepare("SELECT * FROM example").unwrap();
//!    let columns = columns_from_statement(&statement);
//!    let mut res = from_rows_with_columns::<Example, _>(statement.query(NO_PARAMS).unwrap(), &columns);
//!    assert_eq!(res.next().unwrap(), row1);
//!    assert_eq!(res.next().unwrap(), Example{ id: 2, name: "second name".into() });
//!
//!    // deserializing using query_map() and from_row_with_columns()
//!    let mut statement = connection.prepare("SELECT * FROM example").unwrap();
//!    let columns = columns_from_statement(&statement);
//!    let mut rows = statement.query_map(NO_PARAMS, |row| from_row_with_columns::<Example, _>(row, &columns).unwrap()).unwrap();
//!    assert_eq!(rows.next().unwrap().unwrap(), row1);
//!    assert_eq!(rows.next().unwrap().unwrap(), Example{ id: 2, name: "second name".into() });
//!
//!    // deserializing using query() and from_rows_ref_with_columns()
//!    let mut statement = connection.prepare("SELECT * FROM example").unwrap();
//!    let columns = columns_from_statement(&statement);
//!    let mut rows = statement.query(NO_PARAMS).unwrap();
//!    {
//!       // only first record is deserialized here
//!       let mut res = from_rows_ref_with_columns::<Example, _>(&mut rows, &columns);
//!       assert_eq!(res.next().unwrap(), row1);
//!    }
//!    // the second record is deserialized using the original Rows iterator
//!    assert_eq!(from_row_with_columns::<Example, _>(&rows.next().unwrap().unwrap(), &columns).unwrap(), Example{ id: 2, name: "second name".into() });
//!
//! }
//! ```

#[macro_use]
extern crate error_chain;
#[cfg(test)]
#[macro_use]
extern crate matches;
extern crate rusqlite;
#[macro_use]
extern crate serde;
#[cfg(test)]
#[macro_use]
extern crate serde_derive;

pub mod error;
pub mod de;
pub mod ser;
#[cfg(test)]
mod tests;

pub use de::RowDeserializer;
pub use error::{Error, ErrorKind, Result};
pub use ser::{NamedParamSlice, NamedSliceSerializer, PositionalParamSlice, PositionalSliceSerializer};
use std::marker;

/// Iterator to automatically deserialize each row from owned `rusqlite::Rows` into `D: serde::Deserialize`
pub struct DeserRows<'rows, D, S: 'rows> {
	rows: rusqlite::Rows<'rows>,
	columns: Option<&'rows [S]>,
	d: marker::PhantomData<*const D>,
}

impl<'rows, D: serde::de::DeserializeOwned, S: AsRef<str>> Iterator for DeserRows<'rows, D, S> {
	type Item = D;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(Ok(row)) = self.rows.next() {
			match self.columns {
				Some(columns) => from_row_with_columns(&row, columns).ok(),
				None => from_row(&row).ok(),
			}
		} else {
			None
		}
	}
}

/// Iterator to automatically deserialize each row from borrowed `rusqlite::Rows` into `D: serde::Deserialize`
pub struct DeserRowsRef<'rows, 'stmt: 'rows, D, S: 'rows> {
	rows: &'rows mut rusqlite::Rows<'stmt>,
	columns: Option<&'rows [S]>,
	d: marker::PhantomData<*const D>,
}

impl<'rows, 'stmt, D: serde::de::DeserializeOwned, S: AsRef<str>> Iterator for DeserRowsRef<'rows, 'stmt, D, S> {
	type Item = D;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(Ok(row)) = self.rows.next() {
			match self.columns {
				Some(columns) => from_row_with_columns(&row, columns).ok(),
				None => from_row(&row).ok(),
			}
		} else {
			None
		}
	}
}

/// Returns column names of the statement the way `*_with_columns()` method family expects them
///
/// This function is needed because by default `column_names()` returns `Vec<&str>` which
/// ties it to the lifetime of the `Statement`. This way we won't be able to run for example
/// `.query_map()` because it mutably borrows `Statement` and by that time it's already borrowed
/// for columns. So this function owns all column names to detach them from the lifetime of `Statement`.
pub fn columns_from_statement(stmt: &rusqlite::Statement) -> Vec<String> {
	stmt.column_names().into_iter().map(str::to_owned).collect()
}

/// Deserializes an instance of `D: serde::Deserialize` from `rusqlite::Row`. This function will not work for
/// deserialization into types that need field names, but don't carry them on their own, e.g. `Map`.
///
/// You should supply this function to `query_map()`
pub fn from_row<'row, D: serde::de::DeserializeOwned>(row: &'row rusqlite::Row) -> Result<D> {
	D::deserialize(RowDeserializer::from_row(row))
}

/// Deserializes any instance of `D: serde::Deserialize` from `rusqlite::Row` with specified columns. You need to call
/// this version if you're deserializing into e.g. `Map` because it needs field names, but doesn't specify them. Also
/// note that `columns` will only be used in this case, otherwise `D` type field information will be used.
///
/// You should use this function in the closure you supply to `query_map()`
pub fn from_row_with_columns<'row, D: serde::de::DeserializeOwned, S: AsRef<str>>(row: &'row rusqlite::Row, columns: &'row [S]) -> Result<D> {
	let columns_ref = columns.iter().map(AsRef::as_ref).collect::<Vec<_>>();
	D::deserialize(RowDeserializer::from_row_with_columns(row, &columns_ref))
}

/// Returns iterator that owns `rusqlite::Rows` and deserializes all records from it into instances of `D: serde::Deserialize`.
/// Also see `from_row()` for some specific info.
///
/// This function covers most of the use cases and is easier to use than the alternative `from_rows_ref_with_columns()`.
pub fn from_rows<'rows, D: serde::de::DeserializeOwned>(rows: rusqlite::Rows<'rows>) -> DeserRows<'rows, D, String> {
	DeserRows { rows, columns: None, d: marker::PhantomData }
}

/// Returns iterator that owns `rusqlite::Rows` and deserializes all records from it into instances of `D: serde::Deserialize`.
/// Also see `from_row_with_columns()` for some specific info.
///
/// This function covers most of the use cases and is easier to use than the alternative `from_rows_ref_with_columns()`.
pub fn from_rows_with_columns<'rows, D: serde::de::DeserializeOwned, S: AsRef<str>>(rows: rusqlite::Rows<'rows>, columns: &'rows [S]) -> DeserRows<'rows, D, S> {
	DeserRows { rows, columns: Some(columns), d: marker::PhantomData }
}

/// Returns iterator that borrows `rusqlite::Rows` and deserializes records from it into instances of `D: serde::Deserialize`
///
/// Use this function instead of `from_rows()` when you still need iterator with the remaining rows after deserializing some
/// of them.
pub fn from_rows_ref<'rows, 'stmt, D: serde::de::DeserializeOwned>(rows: &'rows mut rusqlite::Rows<'stmt>) -> DeserRowsRef<'rows, 'stmt, D, String> {
	DeserRowsRef { rows, columns: None, d: marker::PhantomData }
}

/// Returns iterator that borrows `rusqlite::Rows` and deserializes records from it into instances of `D: serde::Deserialize`
///
/// Use this function instead of `from_rows_with_columns()` when you still need iterator with the remaining rows after
/// deserializing some of them.
pub fn from_rows_ref_with_columns<'rows, 'stmt, D: serde::de::DeserializeOwned, S: AsRef<str>>(rows: &'rows mut rusqlite::Rows<'stmt>, columns: &'rows [S]) -> DeserRowsRef<'rows, 'stmt, D, S> {
	DeserRowsRef { rows, columns: Some(columns), d: marker::PhantomData }
}

/// Serializes an instance of `S: serde::Serialize` into structure for positional bound query arguments
///
/// To get the slice suitable for supplying to `query()` or `execute()` call `to_slice()` on the `Ok` result and
/// borrow it.
pub fn to_params<S: serde::Serialize>(obj: S) -> Result<PositionalParamSlice> {
	obj.serialize(PositionalSliceSerializer::new())
}

/// Serializes an instance of `S: serde::Serialize` into structure for named bound query arguments
///
/// To get the slice suitable for supplying to `query_named()` or `execute_named()` call `to_slice()` on the `Ok` result
/// and borrow it.
pub fn to_params_named<S: serde::Serialize>(obj: S) -> Result<NamedParamSlice> {
	obj.serialize(NamedSliceSerializer::new())
}

