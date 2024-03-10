# inline-sql

Write SQL queries inside Rust functions.

The `inline-sql` crate lets you annotate a function to directly write an SQL query as the function body.
You can use the function parameters as placeholders in your query,
and the query result will automatically be converted to the return type.

See the documentation of the [`#[inline_sql]`][`inline_sql`] macro for more details and examples.

Currently, only [`tokio-postgres`][`tokio_postgres`] is supported as backend.

## Example: Return a [`Vec`] of rows.
```rust
use inline_sql::inline_sql;

#[inline_sql]
async fn get_pets_by_species(
  client: &tokio_postgres::Client,
  species: &str,
) -> Result<Vec<Pet>, tokio_postgres::Error> {
    query!(SELECT * FROM pets WHERE species = $species)
}
```

## Planned features:
* Support for more backends, including synchronous backends.
* Parsing the function arguments to determine the name of the `client` object.
* Support for queries that return exactly one row or an error.
* More attribute arguments to allow:
  * Specifying the query type instead of deducing it from the return type.
  * Changing how the `client` is obtained in the generated code (for example, from a member of `self`).

[`inline_sql`]: https://docs.rs/inline-sql/latest/inline_sql/attr.inline_sql.html
[`tokio_postgres`]: https://docs.rs/tokio-postgres
[`Vec`]: https://doc.rust-lang.org/stable/std/vec/struct.Vec.html
[`Option`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html
