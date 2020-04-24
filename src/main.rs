#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate juniper; // = import external crate? why do I not need rocket, etc?

mod db;
mod model; // = include contents of model module here
mod schema;

use db::{Database, ManagedDatabase};
use juniper::{EmptyMutation, RootNode};
use rocket::response::content;
use rocket::State;
use schema::Query;
use std::sync::{Arc, Mutex};
use sysinfo::SystemExt;

// declare the schema, will need to dig into this more
// Presumably this would use not an Empty Mutation in the future
type Schema = RootNode<'static, Query, EmptyMutation<Database>>;

// ROCKET ROUTES

#[rocket::get("/")]
fn root() -> content::Html<String> {
    content::Html("Welcome to Kraken!".to_string())
}

#[rocket::get("/graphiql")]
fn graphiql() -> content::Html<String> {
    juniper_rocket::graphiql_source("/graphql")
}

#[rocket::get("/graphql?<request>")]
fn get_graphql_handler(
    context: State<ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<Schema>,
) -> juniper_rocket::GraphQLResponse {
    let ctx = context.db.lock().unwrap();
    request.execute(&schema, &ctx)
}

#[rocket::post("/graphql", data = "<request>")]
fn post_graphql_handler(
    context: State<ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<Schema>,
) -> juniper_rocket::GraphQLResponse {
    let ctx = context.db.lock().unwrap();
    request.execute(&schema, &ctx)
}

fn main() {
    // Manage binds data to the State guard
    // each manage must be to a unique T, so that is can be matched?
    // these are then passed to the routes if they ask for them
    // TODO: use this to manage RabbitMQ messages

    //let mdb = ManagedDatabase::new(Database::new());

    let db = Database::new();

    let db_ref = Arc::new(Mutex::new(db));
    let child_proc;
    {
        let arc = db_ref.clone();
        child_proc = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::new(5, 0));
            let mut system = sysinfo::System::new();
            //let sys = sysinfo::linux::system::System::Processor::get_cpu_usage();
            system.refresh_all();
            println!("total memory: {} kB", system.get_total_memory());
            println!("used memory : {} kB", system.get_used_memory());
            let mut db = arc.lock().unwrap();
            if let Some(n) = db.get_node("test") {
                let mut new_node = n.clone();
                new_node.set_disk_ram_cpu(12.0, 12, 12);
                db.insert_node("test", new_node);
            }
        });
    }

    rocket::ignite()
        .manage(ManagedDatabase::new(db_ref.clone()))
        .manage(Schema::new(Query, EmptyMutation::<Database>::new()))
        .mount(
            "/",
            rocket::routes![root, graphiql, get_graphql_handler, post_graphql_handler],
        )
        .launch();

    child_proc.join().unwrap();
}
