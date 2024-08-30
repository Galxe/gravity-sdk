// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::network_controller::{metrics::NETWORK_HANDLER_TIMER, Message, MessageType};
use aptos_logger::{error, info};
// use aptos_protos::remote_executor::v1::{
//     network_message_service_client::NetworkMessageServiceClient,
//     network_message_service_server::{NetworkMessageService, NetworkMessageServiceServer},
//     Empty, NetworkMessage, FILE_DESCRIPTOR_SET,
// };
use crossbeam_channel::Sender;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{runtime::Runtime, sync::oneshot};
use tonic::{
    transport::{Channel, Server},
    Request, Response, Status,
};

const MAX_MESSAGE_SIZE: usize = 1024 * 1024 * 80;

pub struct GRPCNetworkMessageServiceServerWrapper {
    inbound_handlers: Arc<Mutex<HashMap<MessageType, Sender<Message>>>>,
    self_addr: SocketAddr,
}

impl GRPCNetworkMessageServiceServerWrapper {
    pub fn new(
        inbound_handlers: Arc<Mutex<HashMap<MessageType, Sender<Message>>>>,
        self_addr: SocketAddr,
    ) -> Self {
        Self {
            inbound_handlers,
            self_addr,
        }
    }

    // Note: The object is consumed here. That is once the server is started, we cannot/should not
    //       use the object anymore
    pub fn start(
        self,
        rt: &Runtime,
        _service: String,
        server_addr: SocketAddr,
        rpc_timeout_ms: u64,
        server_shutdown_rx: oneshot::Receiver<()>,
    ) {
        rt.spawn(async move {
            self.start_async(server_addr, rpc_timeout_ms, server_shutdown_rx)
                .await;
        });
    }

    async fn start_async(
        self,
        server_addr: SocketAddr,
        rpc_timeout_ms: u64,
        server_shutdown_rx: oneshot::Receiver<()>,
    ) {
        todo!()
    }
}
