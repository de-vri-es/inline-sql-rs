use inline_sql::inline_sql;
use clap::CommandFactory;

#[derive(clap::Parser)]
struct Options {
	#[clap(long, short)]
	#[clap(global = true)]
	url: Option<String>,

	#[clap(subcommand)]
	command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
	CreateTable,
	GetPets,
	GetPet(GetPet),
	AddPet(AddPet),
}

#[derive(clap::Args)]
struct GetPet {
	name: String,
}

#[derive(clap::Args)]
struct AddPet {
	name: String,
	species: String,
}

#[inline_sql]
async fn create_table(client: &tokio_postgres::Client) -> Result<u64, tokio_postgres::Error> {
	query!(CREATE TABLE pets (
		name TEXT PRIMARY KEY,
		species TEXT NOT NULL
	))
}

#[inline_sql]
async fn get_pets(client: &tokio_postgres::Client) -> Result<Vec<Pet>, tokio_postgres::Error> {
	query!(SELECT * FROM pets)
}

#[inline_sql]
async fn get_pet_by_name(client: &tokio_postgres::Client, name: &str) -> Result<Option<Pet>, tokio_postgres::Error> {
	query!(SELECT * FROM pets WHERE name = #name)
}

#[inline_sql]
async fn add_pet(client: &tokio_postgres::Client, name: &str, species: &str) -> Result<u64, tokio_postgres::Error> {
	query!(INSERT INTO pets (name, species) VALUES (#name, #species))
}

#[derive(pg_mapper::TryFromRow)]
#[derive(Debug)]
struct Pet {
	name: String,
	species: String,
}

#[tokio::main]
async fn main() {
	if let Err(()) = do_main(clap::Parser::parse()).await {
		std::process::exit(1);
	}
}

async fn do_main(options: Options) -> Result<(), ()> {
	let url = options.url
		.ok_or_else(|| {
			clap::error::Error::<clap::error::RichFormatter>::raw(
				clap::error::ErrorKind::MissingRequiredArgument,
				"the following required argument is missing: --url URL\n",
			)
			.with_cmd(&Options::command())
			.print()
			.ok();
		})?;

	let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
		.await
		.map_err(|e| eprintln!("Failed to connect to {url}: {e}"))?;
	let connection = async move {
		connection.await.map_err(|e| println!("Error in connection with postgres: {e}"))
	};

	let work = async move {
		match options.command {
			Command::CreateTable => {
				create_table(&client)
					.await
					.map_err(|e| eprintln!("Failed to create table: {e}"))?;
				Ok(())
			},
			Command::GetPets => {
				let pets = get_pets(&client)
					.await
					.map_err(|e| eprintln!("Failed to get pets: {e}"))?;
				println!("{:#?}", pets);
				Ok(())
			},
			Command::GetPet(command) => {
				let pet = get_pet_by_name(&client, &command.name)
					.await
					.map_err(|e| eprintln!("Failed to get pet: {e}"))?
					.ok_or_else(|| eprintln!("No pet found with name: {:?}", command.name))?;
				println!("{pet:#?}");
				Ok(())
			},
			Command::AddPet(command) => {
				let count = add_pet(&client, &command.name, &command.species)
					.await
					.map_err(|e| eprintln!("Failed to insert pet: {e}"))?;
				println!("Inserted {count} rows" );
				Ok(())
			},
		}
	};

	tokio::try_join!(connection, work)?;
	Ok(())
}
