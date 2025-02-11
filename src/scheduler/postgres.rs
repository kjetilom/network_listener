pub mod postgres_backend {
    use tokio_postgres::{NoTls, Client};

    pub async fn insert_metric(measurement: &str, value: f64, tags: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (client, connection) = tokio_postgres::connect(
            "host=localhost user=user password=password dbname=metricsdb",
            NoTls
        ).await?;

        // Run the connection in the background.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        client.execute("INSERT INTO metrics (measurement, value, tags) VALUES ($1, $2, $3)",
                       &[&measurement, &value, &tags]).await?;

        Ok(())
    }
}