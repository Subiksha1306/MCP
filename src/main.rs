use tokio::net::TcpListener;
use mcp::server::create_router;

mod mcp;
mod connectors;
mod tools;

#[tokio::main]
async fn main() {
    let app = create_router();

    let listener = TcpListener::bind("127.0.0.1:3721")
        .await
        .unwrap();

    println!("🚀 Server running on http://127.0.0.1:3721");

    axum::serve(listener, app)
        .await
        .unwrap();
}