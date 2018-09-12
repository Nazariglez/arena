extern crate arena_core;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;


use arena_core::*;

#[derive(Serialize)]
struct State1 {
    value: i32
}

impl State for State1 {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }

    fn on_create(&mut self) {
        self.value = 2000;
    }
}

pub fn main() {
    run(|| {
        let mut server = Server::new();
        server.add_room("my_room1", Room::with_state(Box::new(State1 {value: 20})));
        server.to_json();
        server.remove_room("my_room1");
        server.to_json();
        
        server
    });
}