//! Write SQL queries inside Rust functions.
//!
//! The `inline-sql` crate lets you annotate a function to directly write an SQL query as the function body.
//! You can use the function parameters as placeholders in your query,
//! and the query result will automatically be converted to the return type.
//!
//! See the documentation of the [`#[inline_sql]`][`inline_sql`] macro for more details and examples.
//!
//! Currently, only [`tokio-postgres`][`tokio_postgres`] is supported as backend.
//!
//! # Example: Return a [`Vec`] of rows.
//! ```
//! # #[derive(pg_mapper::TryFromRow)]
//! # struct Pet {
//! #   name: String,
//! #   species: String,
//! # }
//! use inline_sql::inline_sql;
//!
//! #[inline_sql]
//! async fn get_pets_by_species(
//!   client: &tokio_postgres::Client,
//!   species: &str,
//! ) -> Result<Vec<Pet>, tokio_postgres::Error> {
//!     query!(SELECT * FROM pets WHERE species = $species)
//! }
//! ```
//!
//! # Planned features:
//! * Support for more backends, including synchronous backends.
//! * Parsing the function arguments to determine the name of the `client` object.
//! * Support for queries that return exactly one row or an error.
//! * More attribute arguments to allow:
//!   * Specifying the query type instead of deducing it from the return type.
//!   * Changing how the `client` is obtained in the generated code (for example, from a member of `self`).

#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]


/// Mark a function that executes an SQL query.
///
/// The function body must follow the form
/// `query! { ... }` or `query!(...)`.
///
/// The return type of the function determines the behavior of the function.
/// There are a few options for the return type:
/// * [`Result`]`<(), E>`: Execute the query without returning anything. Errors are still reported.
/// * [`Result`]`<`[`u64`]`, E>`: Execute the query and return the number of affected rows.
/// * [`Result`]`<`[`Vec`]`<T>, E>`: Execute the query and return the rows as a vector.
/// * [`Result`]`<`[`Option`]`<T>, E>`: Execute the query and return a single optional row.
/// * [`Result`]`<`[`RowStream`][`tokio_postgres::RowStream`]`, E>`: Execute the query and return a [`RowStream`][`tokio_postgres::RowStream`].
///
/// The row type `T` must implement [`TryFrom<`][TryFrom][`tokio_postgres::Row`]`>`.
/// The [`TryFrom::Error`] type must implement [`Into<E>`].
///
/// The error type `E` must implement [`From<`][From][`tokio_postgres::Error`]>`.
///
/// For functions that return a `Result<Option<T>, E>`, an error is reported if the query returned more than one row.
///
/// You can generally not use a type alias in the return type of the function.
/// The proc macro can not resolve the alias, and will not know which variant to generate.
///
/// # Macro arguments
///
/// The attribute macro also accepts a arguments.
/// Multiple arguments may be specified separated by a comma.
///
/// ## `#[inline_sql(map_row = ...)]`
/// The `map_row` argument specifies the function that will be called on a row to convert it to the desired type.
/// This function signature must be `Fn(tokio_postgres::Row) -> Result<T, E>`.
///
/// You can specify the name of a function or a lambda.
///
/// Defaults to the equivalent of `|row| TryFrom::try_from(row)?` if not specified.
///
/// ## `#[inline_sql(map_err = ...)]`
/// The `map_err` argument specifies the function that will be called to convert the SQL error to the user error type.
/// This function signature must be `FnOnce(tokio_postgres::Error) -> E`.
///
/// You can specify the name of a function or a lambda.
///
/// Defaults to `TryFrom::try_from` if not specified.
///
/// # Example 1: Ignore the query output.
/// ```
/// use inline_sql::inline_sql;
///
/// #[inline_sql]
/// async fn create_pets_table(
///   client: &tokio_postgres::Client
/// ) -> Result<(), tokio_postgres::Error> {
///   query! {
///     CREATE TABLE pets (
///       name TEXT PRIMARY KEY,
///       species TEXT NOT NULL
///     )
///   }
/// }
/// ```
///
/// # Example: Return a [`Vec`] of rows.
/// ```
/// use inline_sql::inline_sql;
///
/// # #[derive(pg_mapper::TryFromRow)]
/// # struct Pet {
/// #   name: String,
/// #   species: String,
/// # }
///
/// #[inline_sql]
/// async fn get_pets_by_species(
///   client: &tokio_postgres::Client,
///   species: &str,
/// ) -> Result<Vec<Pet>, tokio_postgres::Error> {
///     query!(SELECT * FROM pets WHERE species = #species)
/// }
/// ```
///
/// # Example: Return an [`Option`].
/// ```
/// use inline_sql::inline_sql;
///
/// # #[derive(pg_mapper::TryFromRow)]
/// # struct Pet {
/// #   name: String,
/// #   species: String,
/// # }
///
/// #[inline_sql]
/// async fn get_pet_by_name(
///   client: &tokio_postgres::Client,
///   name: &str,
/// ) -> Result<Option<Pet>, tokio_postgres::Error> {
///     query!(SELECT * FROM pets WHERE name = #name)
/// }
/// ```
///
/// # Example: Return the number of affected rows.
/// ```
/// use inline_sql::inline_sql;
///
/// # #[derive(pg_mapper::TryFromRow)]
/// # struct Pet {
/// #   name: String,
/// #   species: String,
/// # }
///
/// #[inline_sql]
/// async fn rename_species(
///   client: &tokio_postgres::Client,
///   old_species: &str,
///   new_species: &str,
/// ) -> Result<u64, tokio_postgres::Error> {
///     query!(UPDATE pets SET species = #new_species WHERE species = #old_species)
/// }
/// ```
pub use inline_sql_macros::inline_sql;

#[doc(hidden)]
pub mod macro_export__ {
	pub mod prelude {
		pub use futures::StreamExt;
	}

	pub fn convert_row<F, T, E>(fun: F, row: tokio_postgres::Row) -> Result<T, E>
	where
		F: Fn(tokio_postgres::Row) -> Result<T, E>
	{
		(fun)(row)
	}
}
