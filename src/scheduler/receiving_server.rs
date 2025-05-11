use std::time::Duration;

use crate::proto_bw::{DataMsg, HelloMessage};
use crate::proto_bw::client_data_service_server::{ClientDataService, ClientDataServiceServer};
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tonic::transport::Server;
use tonic::{Request, Response, Status, Streaming};
use anyhow::Result;


#[derive(Debug)]
pub struct DataReceiver {
    data_tx: Sender<DataMsg>,
}

impl DataReceiver {
    pub fn new(data_tx: Sender<DataMsg> ) -> Self {
        DataReceiver { data_tx }
    }

    /// Spawns the server in the background.
    /// Consumes self, returns a handle to the task
    pub fn dispatch_server(self, listen_addr: String) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let addr = listen_addr.parse().expect("Failed to parse address");

            Server::builder()
                .add_service(ClientDataServiceServer::new(self))
                .serve(addr)
                .await?;
            Ok(())
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