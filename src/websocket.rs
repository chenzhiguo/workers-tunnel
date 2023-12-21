use futures_util::Stream;
use std::{
    io::{Error, ErrorKind, Result},
    pin::Pin,
    task::{Context, Poll},
};

use bytes::{BufMut, BytesMut};
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use worker::{EventStream, WebSocket, WebsocketEvent};

#[pin_project]
pub struct WebSocketConnection<'a> {
    ws: &'a WebSocket,
    #[pin]
    stream: EventStream<'a>,
    buffer: BytesMut,
    init_write: bool,
}

impl<'a> WebSocketConnection<'a> {
    pub fn new(
        ws: &'a WebSocket,
        stream: EventStream<'a>,
        early_data: Option<Vec<u8>>,
    ) -> Self {
        let mut buff = BytesMut::with_capacity(4096);
        if let Some(data) = early_data {
            buff.put_slice(&data)
        }

        Self {
            ws,
            stream,
            buffer: buff,
            init_write: false,
        }
    }
}

impl<'a> AsyncRead for WebSocketConnection<'a> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let this = self.project();

        let amt = std::cmp::min(this.buffer.len(), buf.remaining());
        if amt > 0 {
            buf.put_slice(&this.buffer.split_to(amt));
            return Poll::Ready(Ok(()));
        }

        match this.stream.poll_next(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Some(Ok(WebsocketEvent::Message(msg)))) => {
                if let Some(data) = msg.bytes() {
                    this.buffer.put_slice(&data);

                    let amt = std::cmp::min(this.buffer.len(), buf.remaining());
                    if amt > 0 {
                        buf.put_slice(&this.buffer.split_to(amt));
                    }
                };
                return Poll::Ready(Ok(()));
            }
            Poll::Ready(None) | Poll::Ready(Some(Ok(WebsocketEvent::Close(_)))) => {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, "Connection closed")))
            }
            Poll::Ready(Some(Err(e))) => {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, e.to_string())))
            }
        }
    }
}

impl<'a> AsyncWrite for WebSocketConnection<'a> {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        let this = self.project();
        if !*this.init_write {
            // 发送第一个包时需要加上 vless 的协议 response 头
            *this.init_write = true;

            return match this
                .ws
                .send_with_bytes([&[0u8, 0u8], buf].concat().to_vec().as_slice())
            {
                Ok(()) => Poll::Ready(Ok(buf.len())),
                Err(e) => Poll::Ready(Err(Error::new(ErrorKind::Other, e.to_string()))),
            };
        }

        match this.ws.send_with_bytes(buf) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::Other, e.to_string()))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        let this = self.project();
        match this.ws.close(None, Some("Normal close")) {
            Ok(()) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::Other, e.to_string()))),
        }
    }
}