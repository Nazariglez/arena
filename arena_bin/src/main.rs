extern crate arena_core;

use arena_core::*;

struct MyGameRoom {
    value: i32 //dummy
}

impl Room for MyGameRoom {}

fn main() {
    let mut s = Server::new();
    s.add("my_room", Box::new(MyGameRoom {value: 10}));
}
