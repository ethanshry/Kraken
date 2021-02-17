//! Module containing definitions of rocket routes

use crate::{
    gql_schema::{Mutation, Query},
    platform_executor::orchestration_executor::db::ManagedDatabase,
}; // Include module from the crate

use juniper::{EmptySubscription, RootNode};
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
pub type Schema = RootNode<'static, Query, Mutation, EmptySubscription<ManagedDatabase>>;

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

/// Match to worker healthcheck requests
#[rocket::get("/health/<requester_node_id>", rank = 2)]
pub fn health(
    context: State<'_, ManagedDatabase>,
    requester_node_id: &RawStr,
) -> content::Plain<String> {
    let mut db = context.db.lock().unwrap();
    let orch_priority = db.get_orchestrator_rank(requester_node_id);
    content::Plain(format!("{}", orch_priority))
}

/// Visible graphql editor
#[rocket::get("/graphiql", rank = 2)]
pub fn graphiql() -> content::Html<String> {
    juniper_rocket::graphiql_source("/graphql", None)
}

/// Handle graphql GET requests
#[rocket::get("/graphql?<request>", rank = 2)]
pub fn get_graphql_handler(
    context: State<'_, ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<'_, Schema>,
) -> juniper_rocket::GraphQLResponse {
    request.execute_sync(&schema, &context)
}

/// Handle graphql POST requests
#[rocket::post("/graphql", data = "<request>", rank = 2)]
pub fn post_graphql_handler(
    context: State<'_, ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<'_, Schema>,
) -> juniper_rocket::GraphQLResponse {
    request.execute_sync(&schema, &context)
}

/// Match to requests for log files
#[rocket::get("/log/<log_id>", rank = 2)]
pub fn logs(log_id: &RawStr) -> Result<NamedFile, NotFound<String>> {
    println!(
        "{}",
        format!(
            "{}/{}/{}.log",
            env!("CARGO_MANIFEST_DIR"),
            crate::utils::LOG_LOCATION,
            log_id
        )
    );
    NamedFile::open(format!(
        "{}/{}/{}.log",
        env!("CARGO_MANIFEST_DIR"),
        crate::utils::LOG_LOCATION,
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
