extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate downcast_rs;

use downcast_rs::Downcast;
use serde::Serialize;
use std::collections::HashMap;
use std::any::Any;
use std::sync::Arc;
use std::sync::Mutex;

pub use serde_json::{Value as JsonValue};

pub fn run<F: FnOnce() -> Server>(f:F) {
    let server = f();
    server.run();
}

pub struct Server {
    rooms: HashMap<String, Room>
}

impl Server {
    pub fn new() -> Server {
        Server {
            rooms: HashMap::new()
        }
    }

    pub fn run(&self) {
        println!("run");
    }

    pub fn add_room(&mut self, name: &str, room: Room) {
        self.rooms.insert(name.to_string(), room);
        if let Some(r) = self.get_room(name) {
            r.state.on_create();
        }
    }

    pub fn get_room(&mut self, name: &str) -> Option<&mut Room> {
        self.rooms.get_mut(name)
    }

    pub fn remove_room(&mut self, name: &str) {
        if let Some(mut r) = self.rooms.remove(name) {
            r.state.on_destroy();
        }
    }

    pub fn to_json(&self) {
        println!("Server to_json");
        for (name, room) in self.rooms.iter() {
            println!("{} -> {}", name, room.to_json());
        }
    }
}

pub struct Room {
    state: Box<State>
}

impl Room {
    pub fn new() -> Room {
        Room {
            state: Box::new(EmptyState)
        }
    }

    pub fn with_state(state: Box<State>) -> Room {
        Room {
            state: state
        }
    }

    pub fn state<T: State>(&self) -> Option<&T> {
        self.state.downcast_ref::<T>()
    }

    pub fn state_mut<T: State>(&mut self) -> Option<&mut T> {
        self.state.downcast_mut::<T>()
    }

    pub fn to_json(&self) -> JsonValue {
        self.state.to_json()
    }
}

pub trait State: Downcast/* + Send + Sync*/ {
    fn to_json(&self) -> JsonValue;
    fn on_create(&mut self) {}
    fn on_destroy(&mut self) {}
    fn on_update(&mut self) {}
}

impl_downcast!(State);

#[derive(Serialize)]
struct EmptyState;
impl State for EmptyState {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }
}
