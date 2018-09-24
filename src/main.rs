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
struct State1 {
    value: i32,

    //#[serde(skip_serializing)]
    //serv: Arena,
}

impl State for State1 {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }

    fn on_init(&mut self, room: &mut Room, server: &mut Arena) {
        room.set_max_connections(4);
        println!("on init {}:{}:{:?}", room.name(), room.id(), room.get_max_connections());
        self.value = 2000;
        room.sync(self);

        println!("here1-");
        if let Err(e) = server.add("game_room", Box::new(EmptyState)) {
            println!("ERROR: {}", e);
        }
        println!("here2");
    }

    fn on_destroy(&mut self, room: &mut Room, _server: &mut Arena) {
        println!("on destroy {}:{}", room.name(), room.id());
    }

    fn on_open_connection(&mut self, conn: &str, room: &mut Room, server: &mut Arena) {
        println!("on open connection [{}] {}:{}", conn, room.name(), room.id());
        let conn = room.get_conn(&conn);
        match conn {
            Some(c) => {
                println!("here...");
                let err = server.add_connection_to("game_room", c.clone());
                println!("-- {:?}", err);
            },
            _ => ()
        }
    }
}


pub fn main() {
    let mut server = Arena::new();
    //get(|server|{
    if let Err(e) = server.add("my_room1", Box::new(State1 {value: 20})) {
        println!("ERROR: {}", e);
    }
    println!("len -> {}", server.room_len());
    //server.remove("my_room1");
    let res = server.set_main_room("my_room1");
    println!("{:?}", res);

    println!("len -> {}", server.room_len());
    let s = server.clone();
    thread::spawn(move || {
        for i in 0..10 {
            s.send(Message::OpenConnection(Connection::new()));
            s.send(Message::MsgIn(format!("i:{}", i)));
            thread::sleep_ms(100);
        }
    });

    server.run();

    arena_net::run(8088);
    //});
    //let mut server = Server::new();
}