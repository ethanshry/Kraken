use crate::{
    db::{Database, ManagedDatabase},
    schema::Query,
}; // Include module from the crate

use juniper::{EmptyMutation, RootNode};
use rocket::response::content;
use rocket::State;

// declare the schema, will need to dig into this more
// Presumably this would use not an Empty Mutation in the future
pub type Schema = RootNode<'static, Query, EmptyMutation<Database>>;

// ROCKET ROUTES

#[rocket::get("/")]
pub fn root() -> content::Html<String> {
    content::Html("Welcome to Kraken!".to_string())
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
    let ctx = context.db.lock().unwrap();
    request.execute(&schema, &ctx)
}

#[rocket::post("/graphql", data = "<request>")]
pub fn post_graphql_handler(
    context: State<ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<Schema>,
) -> juniper_rocket::GraphQLResponse {
    let ctx = context.db.lock().unwrap();
    request.execute(&schema, &ctx)
}
