extern crate arena_core;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;


use arena_core::{get_server, State, Room, JsonValue};

#[derive(Serialize)]
struct State1 {
    value: i32
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
    }
}

pub fn main() {
    get_server(|server|{
        server.add("my_room1", Box::new(State1 {value: 20}));
        server.remove("my_room1");
    });
    //let mut server = Server::new();
}