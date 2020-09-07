#![feature(proc_macro_hygiene, decl_macro, async_closure)]

#[macro_use]
extern crate juniper;
extern crate fs_extra;
extern crate rocket;
extern crate rocket_cors;
extern crate serde;
extern crate strum;
extern crate strum_macros;
extern crate tokio;

mod db;
mod docker;
mod file_utils;
mod git_utils;
mod gitapi;
mod model;
mod platform_executor;
mod rabbit;
mod routes;
mod schema;
mod utils;
mod worker;

#[tokio::main]
async fn main() -> Result<(), ()> {
    Ok(())
}
