use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use prometheus::{Encoder, TextEncoder, register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram};
use tokio::time::{self, Duration};
use lazy_static::lazy_static;



async fn metrics_handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", encoder.format_type())
        .body(Body::from(buffer))
        .unwrap())
}

async fn setup_metrics_server() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let make_svc = make_service_fn(|_conn| {
        async {
            Ok::<_, Infallible>(service_fn(metrics_handler))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);
    println!("Serving metrics at http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}