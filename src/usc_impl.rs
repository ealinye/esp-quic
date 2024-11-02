use std::{io, net::SocketAddr, sync::Arc};

use bytes::BytesMut;
use futures::stream::BoxStream;
use quic::qconnection::{
    path::Pathway,
    usc::{self, ControlSocket},
};

struct UscImpl {
    socket: std::net::UdpSocket,
}

impl UscImpl {
    pub fn new(socket: std::net::UdpSocket) -> Self {
        Self { socket }
    }
}

pub fn install() {
    fn bind(addr: SocketAddr) -> io::Result<Arc<dyn usc::ControlSocket>> {
        let socket = std::net::UdpSocket::bind(addr)?;
        Ok(Arc::new(UscImpl::new(socket)))
    }
    usc::UscImpl::install(usc::UscImpl { bind });
}

impl ControlSocket for UscImpl {
    fn poll_send_via(
        &self,
        _cx: &mut std::task::Context,
        iovecs: &[std::io::IoSlice],
        pathway: Pathway,
    ) -> std::task::Poll<std::io::Result<usize>> {
        // sorry
        for iovec in iovecs {
            self.socket.send_to(iovec, pathway.dst_addr())?;
        }
        std::task::Poll::Ready(Ok(iovecs.len()))
    }

    fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.socket.local_addr()
    }

    fn recv_datagrams(&self) -> BoxStream<'_, std::io::Result<(BytesMut, Pathway)>> {
        struct Receiver<'a> {
            socket: &'a std::net::UdpSocket,
        }

        impl futures::Stream for Receiver<'_> {
            type Item = std::io::Result<(BytesMut, Pathway)>;

            fn poll_next(
                self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context,
            ) -> std::task::Poll<Option<Self::Item>> {
                let local_addr = self.socket.local_addr()?;
                let mut buf = [0; 1200];
                let (size, from) = self.socket.recv_from(&mut buf)?;
                let bytes = buf[..size].into();
                let pathway = Pathway::Direct {
                    local: local_addr,
                    remote: from,
                };
                std::task::Poll::Ready(Some(Ok((bytes, pathway))))
            }
        }
        Box::pin(Receiver {
            socket: &self.socket,
        })
    }
}
