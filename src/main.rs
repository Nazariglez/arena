extern crate arena_core;
extern crate arena_net;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;

use arena_core::{Arena, get, State, Room, JsonValue};
use std::thread;

#[derive(Serialize)]
    struct State1 {
        value: i32,

        #[serde(skip_serializing)]
        serv: Arena,
    }

    impl State for State1 {
        fn to_json(&self) -> JsonValue {
            json!(self)
        }

        fn on_init(&mut self, room: &mut Room) {
            println!("on init {}:{}", room.name(), room.id());
            self.value = 2000;
            room.sync(self);

        }

        fn on_destroy(&mut self, room: &mut Room) {
            println!("on destroy {}:{}", room.name(), room.id());
            let s = self.serv.clone();
            self.serv.add("my_room2", Box::new(State1 {value: 30, serv: s.clone()}));
            self.serv.add("my_room3", Box::new(State1 {value: 30, serv: s.clone()}));
            self.serv.add("my_room4", Box::new(State1 {value: 30, serv: s.clone()}));
            self.serv.add("my_room5", Box::new(State1 {value: 30, serv: s}));
        }
    }


pub fn main() {
    let mut server = Arena::new();

    let ss = server.clone();
    //get(|server|{
    server.add("my_room1", Box::new(State1 {value: 20, serv: ss}));
    println!("len -> {}", server.room_len());
    server.remove("my_room1");
    let res = server.set_main_room("my_room1");
    println!("{:?}", res);

    let res = server.add_connection(arena_core::Connection::new());
    println!("{:?}", res);

    println!("len -> {}", server.room_len());
    server.run();

    arena_net::run(8088);
    //});
    //let mut server = Server::new();
}