//! Module containing base definitions of rocket routes

use crate::{
    db::ManagedDatabase,
    schema::{Mutation, Query},
}; // Include module from the crate

use juniper::RootNode;
use rocket::{
    http::RawStr,
    response::status::NotFound,
    response::NamedFile,
    response::{content, Redirect},
    State,
};
use std::path::PathBuf;

// declare the schema, will need to dig into this more
// Presumably this would use not an Empty Mutation in the future
pub type Schema = RootNode<'static, Query, Mutation>;

/// Redirect no path routes to index.html
#[rocket::get("/")]
pub fn root() -> Redirect {
    Redirect::to("/index.html")
}

/// Route used to test api presence
#[rocket::get("/ping", rank = 2)]
pub fn ping() -> content::Plain<String> {
    content::Plain("pong".to_string())
}

/// Visible graphql editor
#[rocket::get("/graphiql", rank = 2)]
pub fn graphiql() -> content::Html<String> {
    juniper_rocket::graphiql_source("/graphql")
}

/// Handle graphql GET requests
#[rocket::get("/graphql?<request>", rank = 2)]
pub fn get_graphql_handler(
    context: State<'_, ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<'_, Schema>,
) -> juniper_rocket::GraphQLResponse {
    request.execute(&schema, &context)
}

/// Handle graphql POST requests
#[rocket::post("/graphql", data = "<request>", rank = 2)]
pub fn post_graphql_handler(
    context: State<'_, ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<'_, Schema>,
) -> juniper_rocket::GraphQLResponse {
    request.execute(&schema, &context)
}

/// Match to requests for log files
/// TODO make based on log location parameter
#[rocket::get("/log/<log_id>", rank = 2)]
pub fn logs(log_id: &RawStr) -> Result<NamedFile, NotFound<String>> {
    println!(
        "{}",
        format!("{}/{}/{}.log", env!("CARGO_MANIFEST_DIR"), "log", log_id)
    );
    NamedFile::open(format!(
        "{}/{}/{}.log",
        env!("CARGO_MANIFEST_DIR"),
        "log",
        log_id
    ))
    .map_err(|e| NotFound(e.to_string()))
}

/// Catchall route, direct remaining queries to the static folder to try to find an asset
#[rocket::get("/<path..>", rank = 4)]
pub fn site(path: PathBuf) -> Result<NamedFile, NotFound<String>> {
    NamedFile::open(format!(
        "{}/{}/{}",
        env!("CARGO_MANIFEST_DIR"),
        "static",
        path.to_str().unwrap()
    ))
    .map_err(|e| NotFound(e.to_string()))
}
