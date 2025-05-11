use std::time::Duration;

use crate::proto_bw::{DataMsg, HelloMessage};
use crate::proto_bw::client_data_service_server::{ClientDataService, ClientDataServiceServer};
use log::{error, info};
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tonic::transport::Server;
use tonic::{Request, Response, Status, Streaming};
use anyhow::Result;


#[derive(Debug, Clone)]
pub struct DataReceiver {
    data_tx: Sender<DataMsg>,
}

impl DataReceiver {
    pub fn new(data_tx: Sender<DataMsg> ) -> Self {
        DataReceiver { data_tx }
    }

    /// Consumes self, returns a handle to the task
    /// Spawns the server in the background.
    /// The server will listen on the address specified in the config file.
    pub fn dispatch_server(self, listen_addr: String) -> JoinHandle<anyhow::Result<()>> {
        tokio::spawn(async move {
            let addr = listen_addr
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid listen address: {}", e))?;

            let mut backoff = Duration::from_secs(3);
            loop {
                info!("Attempting to bind gRPC server on {}", addr);
                let serve_result = Server::builder()
                    .add_service(ClientDataServiceServer::new(self.clone()))
                    .serve(addr);

                match serve_result.await {
                    Ok(()) => {
                        info!("gRPC server exited cleanly");
                        return Ok(());
                    }
                    Err(e) => {
                        error!(
                            "gRPC server failed to start/ran into error: {}. \
                             retrying in {:?}…",
                            e, backoff
                        );
                        sleep(backoff).await;
                        // Exponential backoff with a cap of 30 seconds
                        backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
                    }
                }
            }
        })
    }
}

#[tonic::async_trait]
impl ClientDataService for DataReceiver {

    async fn client_stream (
        &self,
        request: Request<Streaming<DataMsg>>,
    ) -> Result<Response<HelloMessage>, Status> {
        let mut stream = request.into_inner();
        while let Some(msg) = stream.message().await? {
            // Send the message back to the main task
            self.data_tx.send_timeout(msg, Duration::from_secs(2)).await
                .map_err(|_| Status::internal("Failed to send message to data receiver"))?;
        }
        Ok(Response::new(HelloMessage { message: "Goodbye!".into() }))
    }

}