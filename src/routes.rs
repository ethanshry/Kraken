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

// ROCKET ROUTES

#[rocket::get("/")]
pub fn root() -> Redirect {
    Redirect::to("/index.html")
}

#[rocket::get("/ping", rank = 2)]
pub fn ping() -> content::Plain<String> {
    // TODO point to static site
    content::Plain("pong".to_string())
}

#[rocket::get("/graphiql", rank = 2)]
pub fn graphiql() -> content::Html<String> {
    juniper_rocket::graphiql_source("/graphql")
}

#[rocket::get("/graphql?<request>", rank = 2)]
pub fn get_graphql_handler(
    context: State<ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<Schema>,
) -> juniper_rocket::GraphQLResponse {
    request.execute(&schema, &context)
}

#[rocket::post("/graphql", data = "<request>", rank = 2)]
pub fn post_graphql_handler(
    context: State<ManagedDatabase>,
    request: juniper_rocket::GraphQLRequest,
    schema: State<Schema>,
) -> juniper_rocket::GraphQLResponse {
    //let mut ctx = context.db.lock().unwrap();
    request.execute(&schema, &context)
}

#[rocket::get("/log/<log_id>", rank = 2)]
pub fn logs(log_id: &RawStr) -> Result<NamedFile, NotFound<String>> {
    // TODO handle gracefully
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

#[rocket::get("/log/5a4e1284-1c7f-42bc-92bf-4fc441dad400", rank = 2)]
pub fn logs_2() -> Result<NamedFile, NotFound<String>> {
    // TODO handle gracefully
    println!(
        "{}",
        format!(
            "{}/{}/{}.log",
            env!("CARGO_MANIFEST_DIR"),
            "log",
            "5a4e1284-1c7f-42bc-92bf-4fc441dad400"
        )
    );
    NamedFile::open(format!(
        "{}/{}/{}.log",
        env!("CARGO_MANIFEST_DIR"),
        "log",
        "5a4e1284-1c7f-42bc-92bf-4fc441dad400"
    ))
    .map_err(|e| NotFound(e.to_string()))
}

#[rocket::get("/<path..>", rank = 4)]
pub fn site(path: PathBuf) -> Result<NamedFile, NotFound<String>> {
    // TODO handle gracefully
    NamedFile::open(format!(
        "{}/{}/{}",
        env!("CARGO_MANIFEST_DIR"),
        "static",
        path.to_str().unwrap()
    ))
    .map_err(|e| NotFound(e.to_string()))
}
