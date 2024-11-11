use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use prometheus::{Encoder, TextEncoder, register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram};
use tokio::time::{self, Duration};
use lazy_static::lazy_static;

// Define and register metrics using lazy_static to ensure they are initialized only once
lazy_static! {
    static ref REQUEST_COUNTER: Counter = register_counter!(
        "requests_total",
        "Total number of requests made"
    ).unwrap();

    static ref MEMORY_GAUGE: Gauge = register_gauge!(
        "memory_usage_bytes",
        "Current memory usage in bytes"
    ).unwrap();

    static ref REQUEST_DURATION_HISTOGRAM: Histogram = register_histogram!(
        "request_duration_seconds",
        "Histogram of request processing durations"
    ).unwrap();
}

pub async fn start_client() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Clone metrics to move into the updater task
    let memory_gauge = MEMORY_GAUGE.clone();
    let request_duration_histogram = REQUEST_DURATION_HISTOGRAM.clone();

    // Spawn a background task to simulate metric updates
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;

            // Simulate memory usage update
            let simulated_memory = 500_000_000 + rand::random::<u64>() % 500_000_000; // 500MB to 1GB
            memory_gauge.set(simulated_memory as f64);

            // Simulate request duration
            let simulated_duration = rand::random::<f64>() * 2.0; // 0 to 2 seconds
            request_duration_histogram.observe(simulated_duration);

            println!("Metrics updated: memory_usage_bytes = {}, request_duration_seconds = {}", simulated_memory, simulated_duration);
        }
    });

    // Set up the HTTP server to serve /metrics
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let make_svc = make_service_fn(|_conn| {
        async {
            Ok::<_, Infallible>(service_fn(handle_metrics))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Serving metrics at http://{}", addr);
    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
    Ok(())
}

// Handler for incoming HTTP requests
async fn handle_metrics(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    // Increment the request counter
    REQUEST_COUNTER.inc();

    // Simulate processing duration
    let start = tokio::time::Instant::now();

    // (In a real application, you would handle the request here)

    let duration = start.elapsed().as_secs_f64();
    REQUEST_DURATION_HISTOGRAM.observe(duration);

    // Gather all metrics
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    Ok(Response::new(Body::from(buffer)))
}
