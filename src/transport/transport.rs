use tokio::net::TcpStream;
use crate::error::*;
use futures::prelude::*;
use futures::Future;
use futures::Poll;
use std::pin::Pin;
use futures::task::Context;
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};
use crate::transport::request::*;
use crate::transport::tagstore::*;

pub enum Transport{}

impl Transport {
    pub async fn tcp_connect(address: String) -> Result<(TransportFuture<TcpStream>, TransportClient)>
    {
        let stream = TcpStream::connect(address).await?;
        let (tx ,rx) = tokio::sync::mpsc::unbounded_channel();
        let fut = TransportFuture::new(rx, stream);
        Ok((fut, TransportClient::new(tx)))
    }
}

#[derive(Clone)]
pub struct TransportClient {
    tx: UnboundedSender<Request>,
}

impl TransportClient {
    fn new(tx: UnboundedSender<Request>) -> Self {
        TransportClient {
            tx
        }
    }

    pub async fn query(&mut self, req: &[u8]) -> Result<ResponseStream> {
        let (tx, rx) = unbounded_channel();
        self.tx.send(Request {
            tx,
            data: req.to_vec()
        }).await?;
        trace!("Sending request to request_queue");
        Ok(
            ResponseStream {
                rx
            })
    }
}

#[derive(Debug)]
enum TransportState {
    Pending,
    WritingToBuffer
}

pub struct TransportFuture<S> {
    rx: UnboundedReceiver<Request>,
    request_id: AtomicU16,
    response_id: AtomicU16,
    stream: S,
    tag_store: TagStore,
    state: TransportState,
    write_buffer: Vec<u8>,
    read_buffer: Vec<u8>
}

impl<S> TransportFuture<S>
where S: AsyncRead + AsyncWrite + Unpin
{
    fn new(rx: UnboundedReceiver<Request>, stream: S) -> Self {
        TransportFuture {
            rx,
            request_id: AtomicU16::new(1),
            response_id: AtomicU16::new(1),
            stream,
            state: TransportState::Pending,
            write_buffer: Vec::new(),
            read_buffer: vec![0_u8; 1600],
            tag_store: TagStore::new(100),
        }
    }
    fn write_request(&mut self, req: Request) -> Result<()> {
        let request_id = self.request_id.fetch_add(1, Ordering::SeqCst);
        trace!("write_request: id: {}", request_id);
        self.write_buffer.extend_from_slice(&req.data);
        self.tag_store.add(RequestCallback {
            tx: req.tx,
            tag: request_id,
            total: None,
            sent: None
        });
        trace!("write_request: done");
        Ok(())
    }

    fn read_response(&mut self, bytes_read: usize) -> Result<()> {
        trace!("read_response: Received {} bytes from transport", bytes_read);
        if bytes_read < 8 {
            return Err(ErrorKind::UnknownError(String::from("Invalid header received")).into());
        }
        let buffer = &self.read_buffer[0..bytes_read];
        let res = ResponseChunk {
            total: 1,
            part: 1,
            tag: self.response_id.fetch_add(1, Ordering::SeqCst),
            data: buffer.to_vec()
        };
        let r = self.tag_store.add_response(res);
        trace!("read_response: done");
        r
    }

    fn poll_write(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        trace!("poll_write: buffer size: {}", self.write_buffer.len());
        let poll = Pin::new(&mut self.stream).poll_write(cx, &self.write_buffer);
        match poll {
            Poll::Ready(Ok(bytes_written)) => {
                trace!("poll_write: written {}", bytes_written);
                self.write_buffer.clear();
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                Poll::Ready(Err(From::from(e)))
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let poll = Pin::new(&mut self.stream).poll_flush(cx);
        match poll {
            Poll::Ready(Ok(_)) => {
                self.state = TransportState::Pending;
                Poll::Ready(Ok(()))
            },
            Poll::Ready(Err(e)) => {
                Poll::Ready(Err(From::from(e)))
            },
            Poll::Pending => {
                Poll::Pending
            }
        }
    }

    fn poll_request(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<()>>> {
        trace!("poll_request");
        match ready!(self.rx.poll_recv(cx)) {
            Some(req) => {
                trace!("poll_request: dequeue item");
                self.write_request(req)?;
                self.state = TransportState::WritingToBuffer;
                Poll::Ready(Ok(Some(())))
            },
            None => {
                trace!("poll_request: stream closed");
                Poll::Ready(Ok(None))
            }
        }

    }

    fn poll_response(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        trace!("poll_response");
        match Pin::new(&mut self.stream).poll_read(cx, &mut self.read_buffer) {
            Poll::Ready(Ok(bytes_read)) => {
                trace!("poll_response: read: {}", bytes_read);
                self.read_response(bytes_read)?;
                Poll::Ready(Ok(()))
            },
            Poll::Ready(Err(e)) => {
                Poll::Ready(Err(From::from(e)))
            },
            Poll::Pending => {
                Poll::Pending
            },
        }
    }
}

impl<S> Future for TransportFuture<S>
where S: AsyncRead + AsyncWrite + Unpin
{
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        loop {
            trace!("start state: {:?}", me.state);
            match me.state {
                TransportState::Pending => {
                    trace!("poll_request: start");
                    let mut poll_request_pending = false;
                    let mut poll_response_pending = false;
                    match me.poll_request(cx) {
                        Poll::Ready(Ok(Some(_))) => {
                            trace!("poll_request: done (ok)")
                        },
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Err(e))
                        },
                        Poll::Ready(Ok(None)) => {
                            trace!("poll_request: client closed");
                            if me.tag_store.is_empty() {
                                return Poll::Ready(Ok(()))
                            }
                        },
                        Poll::Pending => {
                            poll_request_pending = true;
                            trace!("poll_request: done (pending)");
                        }
                    }
                    trace!("poll_response: start");
                    match me.poll_response(cx) {
                        Poll::Ready(Ok(_)) => {
                            trace!("poll_response: done (ok)");
                        },
                        Poll::Ready(Err(r)) => {
                            return Poll::Ready(Err(r))
                        },
                        Poll::Pending => {
                            poll_response_pending = true;
                            trace!("poll_response: done (pending)");
                        },
                    }
                    if poll_request_pending && poll_response_pending {
                        return Poll::Pending;
                    }
                },
                TransportState::WritingToBuffer => {
                    trace!("poll_write: start");
                    match me.poll_write(cx) {
                        Poll::Ready(Ok(_)) => {
                            trace!("poll_write: done (ok)");
                        },
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Err(e))
                        },
                        Poll::Pending => {
                            trace!("poll_write: done (pending)");
                            return Poll::Pending;
                        },
                    }
                    trace!("poll_flush: start");
                    match me.poll_flush(cx) {
                        Poll::Ready(Ok(_)) => {
                            trace!("poll_flush: done (ok)");
                        },
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Err(e))
                        },
                        Poll::Pending => {
                            trace!("poll_flush: done (pending)");
                            return Poll::Pending;
                        },
                    }
                }
            }
            trace!("end state: {:?}", me.state);
        }
    }
}