use axum::{
    routing::get,
    routing::post,
    Router,
    response::IntoResponse,
    http::StatusCode,
};
use deezel_cli::runestone_enhanced;
use std::net::SocketAddr;
use std::str::FromStr;
use clap::Parser;
use bdk::bitcoin::consensus::deserialize;
use runestone_enhanced::format_runestone;
use serde_json::json;

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "Service is healthy")
}

async fn decode_runestone(
    tx_hex: String,
) -> impl IntoResponse {
    let tx_bytes = hex::decode(tx_hex).expect("Failed to decode transaction hex");
        
    // Deserialize directly into a BDK transaction
    let bdk_tx: bdk::bitcoin::Transaction = deserialize(&tx_bytes).expect("Failed to deserialize transaction");

    // Try to format the Runestone
    match format_runestone(&bdk_tx) {
        Ok(protostones) => {
            // Convert protostones to a JSON-serializable format
            let protostones_json: Vec<serde_json::Value> = protostones.iter().map(|p| {
                json!({
                    "type": format!("{:?}", p),
                    "burn": p.burn,
                    "message": serde_json::to_string(&p.message).unwrap(),
                    "edicts": p.edicts.iter().map(|e| {
                        json!({
                            "id": json!({
                                "block": e.id.block,
                                "tx": e.id.tx
                            }),
                            "amount": e.amount,
                            "output": e.output
                        })
                    }).collect::<Vec<_>>(),
                    "refund": p.refund,
                    "pointer": p.pointer
                })
            }).collect();

            let response = json!({
                "status": "success",
                "protostones": protostones_json
            });
            (StatusCode::OK, response.to_string())
        }
        Err(e) => {
            let response = json!({
                "status": "error",
                "message": e.to_string()
            });
            (StatusCode::BAD_REQUEST, response.to_string())
        }
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The address to bind the server to (e.g., 127.0.0.1:8080)
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    addr: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = SocketAddr::from_str(&args.addr)?;
    
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/decode", post(decode_runestone));

    println!("Starting HTTP server on {}", addr);
    
    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        app.into_make_service()
    ).await?;

    Ok(())
} 