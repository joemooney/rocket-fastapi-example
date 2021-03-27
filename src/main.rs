#![feature(proc_macro_hygiene, decl_macro)]

/*
This program is an example of a rocket http server that
uses rocket_okapi openapi crate to provide a FastAPI like
implemntation in Rust.

This also shows how to store State information that
persists and is thread safe.
 */
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_okapi;

use std::sync::{Mutex};
use rocket::Rocket;
use rocket::State;
use rocket_contrib::json::Json;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
struct StartRequest {
    path: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
struct LoggingResponse {
    path: Option<String>,
    previous_path: Option<String>,
    active: bool,
    request_status: bool,
    request_message: Option<String>,
}

// PUT is idempotent, repeated calls return same value

#[openapi]
#[post("/start", format = "json", data = "<req>")]
fn start(req: Json<StartRequest>, db: State<Db>) -> Json<LoggingResponse> {
    let mut db = db.lock().unwrap();
    let mut status = true;
    let mut message = Some("Logging started".to_string());
    db.call_count += 1;
    if Some(req.path.clone()) == db.path && db.active {
        status = false;
        message = Some("Already logging to this path".to_string());
    } else {
        db.active = true;
        if ! db.path.is_none() {
            db.previous_path = db.path.clone();
        }
        db.path = Some(req.path.clone());
    }
    Json(LoggingResponse {
        path: db.path.clone(),
        previous_path: db.previous_path.clone(),
        active: db.active,
        request_status: status,
        request_message: message,
    })
}

#[openapi]
#[post("/stop")]
fn stop(db: State<Db>) -> Json<LoggingResponse> {
    let mut db = db.lock().unwrap();
    let mut status = true;
    let mut message = Some("Logging stopped".to_string());
    db.call_count += 1;
    if ! db.active {
        status = false;
        message = Some("No logging was active".to_string());
    } else {
        db.active = true;
        db.previous_path = db.path.clone();
        db.path = None;
        db.active = false;
    }
    Json(LoggingResponse {
        path: db.path.clone(),
        previous_path: db.previous_path.clone(),
        active: db.active,
        request_status: status,
        request_message: message,
    })
}

#[openapi]
#[get("/status", format = "json")]
fn status(db: State<Db>) -> Json<LoggingResponse> {
    let mut db = db.lock().unwrap();
    let message = if db.active {
        Some("Logging active".to_string())
    } else {
        Some("No logging active".to_string())
    };
    db.call_count += 1;
    Json(LoggingResponse {
        path: db.path.clone(),
        previous_path: db.previous_path.clone(),
        active: db.active,
        request_status: true,
        request_message: message,
    })
}

/// A simple in-memory DB to store logging state
type Db = Mutex<LoggerState>;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct LoggerState {
    pub id: u64,
    pub path: Option<String>,
    pub previous_path: Option<String>,
    pub call_count: u32,
    pub active: bool,
}

impl LoggerState {
    fn new() -> LoggerState {
        LoggerState{
            id: 0,
            path: None,
            previous_path: None,
            call_count: 0,
            active: false,
        }
    }
}

fn build_app() -> Rocket {
    rocket::ignite()
        .manage(Mutex::new(LoggerState::new()))
        .mount("/", routes_with_openapi![status, start, stop])
        .mount(
            "/docs/",
            make_swagger_ui(&SwaggerUIConfig {
                url: "../openapi.json".to_owned(),
                ..Default::default()
            }),
        )
}

fn main() {
    build_app().launch();
}

#[cfg(test)]
mod tests {
    use super::build_app;
    use rocket::http::{ContentType, Status};
    use rocket::local::Client;

    #[test]
    fn status() {
        let client = Client::new(build_app()).expect("Could not build app");
        let req = client
            .post("/logging/status")
            .header(ContentType::JSON)
            .body(r#"{"path": "/a/b", "action": "start"}"#);
        let mut resp = req.dispatch();
        assert_eq!(resp.status(), Status::Ok);
        assert_eq!(
            resp.body_string(),
            Some(r#"{"name":"Bob","id":null}"#.to_string())
        );
    }
}
