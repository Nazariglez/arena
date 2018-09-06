extern crate arena_core;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;


use arena_core::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MyGameRoom {
    value: i32 //dummy
}

impl Room for MyGameRoom {}

fn main() {
    let mut s = Server::new();
    s.add("my_room", Box::new(MyGameRoom {value: 10}));
}
