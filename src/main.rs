use actix_web::web::BytesMut;
use actix_web::{
    body::MessageBody, error, http, web, App, Error, HttpResponse, HttpServer, Result, Route,
};
use futures::channel::mpsc;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::thread;

pub static LIST: &[u8; 4] = b"LIST";
struct AppState {
    database: Mutex<HashMap<String, web::BytesMut>>,
}

async fn gettin(path: web::Path<String>, data: web::Data<AppState>) -> HttpResponse {
    let id = path.into_inner();
    let database: MutexGuard<HashMap<String, BytesMut>> = data.database.lock().unwrap();

    if let Some(entry) = database.get(&id) {
        let (tx, rx_body) = mpsc::unbounded();
        thread::spawn(move || loop {
            let _ = tx.unbounded_send(
                entry
                    .clone()
                    .try_into_bytes()
                    .map_err(|_| error::ContentTypeError::ParseError),
            );
        });

        HttpResponse::Ok()
            .insert_header(("Transfer-encoding", "chunked"))
            .streaming(rx_body)
    } else {
        HttpResponse::NotFound().body(format!("Key {id} was not found."))
    }
}

async fn deletin(path: web::Path<String>, data: web::Data<AppState>) -> HttpResponse {
    let mut database = data.database.lock().expect("Mutex panicked.");
    let id = path.into_inner();
    match database.remove(&id) {
        None => HttpResponse::NotFound().body(format!("Key {id} was not found in our database.")),
        Some(_) => HttpResponse::Ok().body(format!("Key {id} succesfully deleted.")),
    }
}

async fn puttin(
    path: web::Path<String>,
    data: web::Data<AppState>,
    mut body: web::Payload,
) -> Result<HttpResponse, Error> {
    let mut database = data.database.lock().expect("Mutex panicked.");
    let path = path.into_inner();
    match database.get(&path) {
        None => {
            database.insert(path.clone(), BytesMut::new());
            std::mem::drop(database);
            while let Some(bytes) = body.next().await {
                // probably illegal mutex manipulation, I need to read it while changing it tho
                let mut database = data.database.lock().expect("Mutex panicked.");
                let entry = database.get_mut(&path).unwrap();
                entry.extend_from_slice(&bytes?);
                std::mem::drop(database);
            }
            Ok(HttpResponse::Ok().body("Data were succesfully put."))
        }
        Some(_) => Ok(HttpResponse::Ok().body("No data were changed")),
    }
}

async fn listin(path: web::Path<String>, data: web::Data<AppState>) -> Result<HttpResponse, Error> {
    let database = data.database.lock().expect("Mutex panicked.");
    let path = path.into_inner();
    let mut result = String::from(format!("List of all paths starting with {path}:\n"));
    for key in database.keys() {
        let prefix = &key[..std::cmp::min(path.len(), key.len())];
        if prefix == path {
            result = result + key + "\n";
        }
    }

    Ok(HttpResponse::Ok().body(result))
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app_datas = web::Data::new(AppState {
        database: Mutex::new(HashMap::with_capacity(10_000)),
    });
    let list_method: http::Method = http::Method::from_bytes(LIST).unwrap();
    HttpServer::new(move || {
        App::new()
            .app_data(app_datas.clone())
            .route("/go/{tail:.*}", web::get().to(gettin))
            .route("/go/{tail:.*}", web::put().to(puttin))
            .route("/go/{tail:.*}", web::delete().to(deletin))
            .route(
                "/go/{tail:.*}",
                Route::new().method(list_method.clone()).to(listin),
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
