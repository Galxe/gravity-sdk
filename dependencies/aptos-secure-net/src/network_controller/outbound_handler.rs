// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    // grpc_network_service::GRPCNetworkMessageServiceClientWrapper,
    network_controller::{
        inbound_handler::InboundHandler, metrics::NETWORK_HANDLER_TIMER, Message, MessageType,
    },
};
use aptos_logger::{info, warn};
use crossbeam_channel::{unbounded, Receiver, Select, Sender};
use std::{
    collections::{HashMap, HashSet},
    mem,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::runtime::Runtime;

pub struct OutboundHandler {
    _service: String,
    remote_addresses: HashSet<SocketAddr>,
    address: SocketAddr,
    // Used to route outgoing messages to correct network client with the correct message type
    handlers: Vec<(Receiver<Message>, SocketAddr, MessageType)>,
    inbound_handler: Arc<Mutex<InboundHandler>>,
}

impl OutboundHandler {
    pub fn new(
        service: String,
        listen_addr: SocketAddr,
        inbound_handler: Arc<Mutex<InboundHandler>>,
    ) -> Self {
        Self {
            _service: service,
            remote_addresses: HashSet::new(),
            address: listen_addr,
            handlers: Vec::new(),
            inbound_handler,
        }
    }

    pub fn register_handler(
        &mut self,
        message_type: String,
        remote_addr: SocketAddr,
        receiver: Receiver<Message>,
    ) {
        self.remote_addresses.insert(remote_addr);
        self.handlers
            .push((receiver, remote_addr, MessageType::new(message_type)));
    }

    pub fn start(&mut self, rt: &Runtime) -> Option<Sender<Message>> {
        todo!()
    }
}
