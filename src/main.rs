use actix_web::web::BytesMut;
use actix_web::{http, web, App, Error, HttpResponse, HttpServer, Result, Route};
use futures::{future::ok, stream::once};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

pub static LIST: &[u8; 4] = b"LIST";
struct AppState {
    database: Mutex<HashMap<String, web::BytesMut>>,
}

async fn gettin(path: web::Path<String>, data: web::Data<AppState>) -> HttpResponse {
    let id = path.into_inner();
    let database: MutexGuard<HashMap<String, BytesMut>> = data.database.lock().unwrap();

    if let Some(entry) = database.get(&id) {
        let body = once(ok::<_, Error>(web::Bytes::from(entry.to_owned())));

        HttpResponse::Ok()
            .insert_header(("Transfer-encoding", "chunked"))
            .streaming(body)
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
            let entry = database.get_mut(&path).unwrap();
            while let Some(bytes) = body.next().await {
                entry.extend_from_slice(&bytes?);
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
