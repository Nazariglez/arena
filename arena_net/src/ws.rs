use ws_rs;
use std::thread;
use arena_core::{Arena, Connection, ClientEvents, JsonValue};

struct WsConn {
    id: Option<String>,
    out: ws_rs::Sender,
    arena: Arena,
}

impl WsConn {
    pub fn new(out: ws_rs::Sender, arena: Arena) -> WsConn {
        WsConn {
            id: None,
            out: out,
            arena: arena
        }
    }
}

impl ws_rs::Handler for WsConn {
    fn on_open(&mut self, _: ws_rs::Handshake) -> ws_rs::Result<()> {
        let r_conn = self.arena.new_conn();
        match r_conn {
            Ok(conn) => {
                self.id = Some(conn.id.clone());

                let out = self.out.clone();
                let send_msg = move |room, evt, data| {
                    if let Err(e) = out.send(format!("{}", json!({
                        "room": room,
                        "event": evt,
                        "data": data
                    }))) {
                        println!("Error: {}",e);
                    }
                };

                let out = self.out.clone();
                thread::spawn(move || {
                    for evt in conn.listen() {
                        use arena_core::ClientEvents::*;
                        
                        match evt {
                            OpenConnection(id) => {
                                send_msg("".to_string(), "init".to_string(), json!({ "id" : id }));
                            },
                            Msg(room_id, msg) => {
                                send_msg(room_id, msg.event, msg.data);
                            },
                            JoinRoom(room_id, error_reason) => {
                                send_msg(
                                    room_id, 
                                    "join_room".to_string(),
                                    json!({
                                        "error": error_reason.unwrap_or("".to_string())
                                    })
                                );
                            },
                            CloseRoom(room_id, error_reason) => {
                                send_msg(
                                    room_id, 
                                    "close_room".to_string(),
                                    json!({
                                        "reason": error_reason
                                    })
                                );
                            },
                            CloseConnection(reason) => {
                                if let Err(e) = out.close_with_reason(
                                    ws_rs::CloseCode::Normal,
                                    reason.unwrap_or("".to_string())
                                ) {
                                    println!("Error: {}", e); //todo improve how the errors are managed
                                }
                            },
                            _ => {}
                        }
                    }
                });
            },
            Err(e) => {
                if let Err(e) = self.out.close_with_reason(ws_rs::CloseCode::Error, e.clone()) {
                    println!("Error: {}", e);
                }
                //return Err(ws_rs::Error::new(ws_rs::ErrorKind::Internal, e));
            }
        }

        Ok(())
    }

    fn on_close(&mut self, _code: ws_rs::CloseCode, _reason: &str) {
        if let Some(id) = &self.id {
            self.arena.remove_connection(id);
        }
    }

    fn on_message(&mut self, message: ws_rs::Message) -> ws_rs::Result<()> { 
        match message {
            ws_rs::Message::Text(msg) => {
                println!("msg received {}",msg);
            },
            _ => {}
        }
        Ok(())
    }
}

pub fn run<F: Fn() -> Arena>(addr: &str, handler: F) {
    let arena = handler();
    let mut arena_mut = arena.clone();

    thread::spawn(move || {
        arena_mut.run();    
    });

    let err = ws_rs::listen(addr, |out| {
        WsConn::new(out, arena.clone())
    });

    if let Err(e) = err {
        println!("Error intiating ws-rs {}", e);
    }
}