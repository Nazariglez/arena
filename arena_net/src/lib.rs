extern crate actix;
extern crate actix_web;
extern crate arena_core;
extern crate nanoid;

use actix::*;
use actix_web::*;
use arena_core::Connection;

fn init_ws(req: &HttpRequest) -> Result<HttpResponse, Error> {
    ws::start(req, WsConnection::new())
}

pub fn run(port: usize) {
    let sys = System::new("Arena");

    server::new(move || {
        App::new()
            .resource("/ws", |r| r.route().f(init_ws))
    }).bind(format!("127.0.0.1:{}", port))
        .unwrap()
        .start();

    println!("running server");
    sys.run();
}

struct WsConnection {
    id: String
}

impl WsConnection {
    fn new() -> WsConnection {
        WsConnection {
            id: nanoid::simple()
        }
    }
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<ws::Message, ws::ProtocolError> for WsConnection {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        println!("{} WEBSOCKET MESSAGE: {:?}", self.id, msg);

        match msg {
            ws::Message::Close(_) => ctx.stop(),
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Text(text) => ctx.text(text),
            ws::Message::Binary(bin) => ctx.binary(bin),
            _ => (),
        }
    }
}