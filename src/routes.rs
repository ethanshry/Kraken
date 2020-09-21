use crate::{
    db::{Database, ManagedDatabase},
    schema::{Mutation, Query},
}; // Include module from the crate

use juniper::{EmptyMutation, RootNode};
use rocket::{response::content, response::NamedFile, State};
use std::path::PathBuf;

// declare the schema, will need to dig into this more
// Presumably this would use not an Empty Mutation in the future
pub type Schema = RootNode<'static, Query, Mutation>;

// ROCKET ROUTES

#[rocket::get("/")]
pub fn root() -> content::Html<String> {
    // TODO point to static site
    content::Html("Waiting for service to download platform...".to_string())
}

#[rocket::get("/ping")]
pub fn ping() -> content::Plain<String> {
    // TODO point to static site
    content::Plain("pong".to_string())
}

#[rocket::get("/graphiql")]
pub fn graphiql() -> content::Html<String> {
    juniper_rocket::graphiql_source("/graphql")
}

#[rocket::get("/graphql?<request>")]
pub fn get_graphql_handler(
    context: State<ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<Schema>,
) -> juniper_rocket::GraphQLResponse {
    request.execute(&schema, &context)
}

#[rocket::post("/graphql", data = "<request>")]
pub fn post_graphql_handler(
    context: State<ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<Schema>,
) -> juniper_rocket::GraphQLResponse {
    //let mut ctx = context.db.lock().unwrap();
    request.execute(&schema, &context)
}

#[rocket::get("/<path..>")]
pub fn site(path: PathBuf) -> NamedFile {
    // TODO handle gracefully
    NamedFile::open(format!(
        "{}/{}/{}",
        env!("CARGO_MANIFEST_DIR"),
        "static",
        path.to_str().unwrap()
    ))
    .unwrap()
}
