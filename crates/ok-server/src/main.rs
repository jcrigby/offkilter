//! offkilter document server.
//!
//! Serves the built web app, stores documents as `.okpart` JSON files, and
//! relays edits between clients editing the same document in real time.
//! Every edit is an `ok_model::Op`; the server applies each op to its own
//! copy of the document (so bad ops are rejected), assigns it a sequence
//! number, and broadcasts it. Clients apply ops in server order.

mod api;
mod auth;
mod live;
mod store;
mod teams;

use std::net::SocketAddr;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let mut data = PathBuf::from("data");
    let mut static_dir: Option<PathBuf> = None;
    let mut port: u16 = 8080;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => data = PathBuf::from(args.next().expect("--data needs a path")),
            "--static" => {
                static_dir = Some(PathBuf::from(args.next().expect("--static needs a path")))
            }
            "--port" => {
                port = args
                    .next()
                    .expect("--port needs a number")
                    .parse()
                    .expect("bad port")
            }
            "--secure-cookies" => auth::set_secure_cookies(true),
            "-h" | "--help" => {
                println!("ok-server [--data DIR] [--static DIR] [--port N] [--secure-cookies]");
                println!("  --secure-cookies  mark session cookies Secure (serve over HTTPS)");
                return;
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let store = store::DocStore::open(&data).expect("open data directory");
    let users = auth::UserStore::open(&data).expect("open data directory");
    let teams = teams::TeamStore::open(&data).expect("open data directory");
    let app = api::router(store, users, teams, static_dir);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    println!(
        "offkilter server listening on http://{addr} (data in {})",
        data.display()
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("serve");
}
