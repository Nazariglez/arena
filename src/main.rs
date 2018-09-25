extern crate arena_core;
extern crate arena_net;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate log;
extern crate env_logger;

use arena_core::{Arena, State, Room, JsonValue, Message, Connection, EmptyState};
use std::thread;

#[derive(Debug, Serialize)]
struct MainRoom;

impl MainRoom {
    pub fn new() -> MainRoom {
        MainRoom
    }
}

impl State for MainRoom {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }

    fn on_open_connection(&mut self, conn: &str, room: &mut Room, server: &mut Arena) {
        println!("on open connection [{}] {}:{}", conn, room.kind(), room.id());
        let conn = room.get_conn(&conn);
        //tic tac toe, create a new room for every two players
        match conn {
            Some(c) => {
                if let Err(e) = server.add_connection_to("game_room", c.clone()) {
                    println!("ERROR adding connection {}", e);
                }
            },
            _ => ()
        }
    }
}

#[derive(Debug, Serialize)]
enum GameToken {
    Empty,
    Player1,
    Player2
}

#[derive(Debug, Serialize)]
struct GameRoom {
    board: [[GameToken; 3]; 3],
}

impl GameRoom {
    pub fn new() -> GameRoom {
        GameRoom {
            board: [
                [ GameToken::Empty, GameToken::Empty, GameToken::Empty ],
                [ GameToken::Empty, GameToken::Empty, GameToken::Empty ],
                [ GameToken::Empty, GameToken::Empty, GameToken::Empty ],
            ]
        }
    }
}

impl State for GameRoom {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }

    fn on_init(&mut self, room: &mut Room, _server: &mut Arena) {
        room.set_max_connections(2);
    }
}


pub fn main() {
    let mut server = Arena::with_main_room("main_room", Box::new(MainRoom::new()));

    /*let s = server.clone();
    thread::spawn(move || {
        for i in 0..10 {
            s.send(Message::OpenConnection(Connection::new()));
            s.send(Message::MsgIn(format!("i:{}", i)));
            thread::sleep_ms(100);
        }
    });*/

    server.run();

    arena_net::run(8088);
}