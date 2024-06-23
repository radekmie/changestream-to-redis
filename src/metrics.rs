use hyper::{body::Incoming, server::conn::http1::Builder, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use lazy_static::lazy_static;
use prometheus::{
    gather, register_int_counter, register_int_gauge, Encoder, IntCounter, IntGauge, TextEncoder,
};
use std::convert::Infallible;
use tokio::{net::TcpListener, spawn};

lazy_static! {
    pub static ref LAST_EVENT_GAUGE: IntGauge =
        register_int_gauge!("last_event", "Timestamp of last MongoDB event").unwrap();
    pub static ref MONGO_COUNTER: IntCounter =
        register_int_counter!("mongo", "Number of MongoDB events").unwrap();
    pub static ref REDIS_COUNTER: IntCounter =
        register_int_counter!("redis", "Number of Redis invokes").unwrap();
}

pub async fn serve(address: String) {
    let listener = TcpListener::bind(address).await.unwrap();
    loop {
        let io = TokioIo::new(listener.accept().await.unwrap().0);
        spawn(async move {
            Builder::new()
                .serve_connection(io, service_fn(server_metrics))
                .await
                .unwrap();
        });
    }
}

async fn server_metrics(_: Request<Incoming>) -> Result<Response<String>, Infallible> {
    let mut buffer = Vec::new();
    TextEncoder::new().encode(&gather(), &mut buffer).unwrap();
    Ok(Response::new(String::from_utf8(buffer).unwrap()))
}
