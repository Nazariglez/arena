extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;

use serde::Serialize;
use serde_json::{Value as JsonValue};
use std::collections::HashMap;
use std::any::Any;

//Manage host, port, rooms, and the ws/tcp connections
pub struct Server {
    rooms: HashMap<String, RoomContainer>
}

impl Server {
    pub fn new() -> Server {
        Server {
            rooms: HashMap::new()
        }
    }

    pub fn add(&mut self, name: &str, room: Box<Room>) {
        self.rooms.insert(name.to_string(), RoomContainer::new(room));
    }

    pub fn remove(&mut self, name: &str) {
        self.rooms.remove(name);
    }
}

//wrapper for objects which implement the trait Room, store states and timeline
pub struct RoomContainer {
    room: Box<Room>,
    state: JsonValue,
    timeline: RoomTimeline
}

impl RoomContainer {
    pub fn new(room: Box<Room>) -> RoomContainer {
        let state = json!(room);
        //let state = json!({}); //json!(room);

        let mut container = RoomContainer {
            room: room,
            state: state,
            timeline: RoomTimeline::new(100)
        };

        container.room.on_add();

        container
    }

    pub fn on_remove(&mut self) {
        self.room.on_remove();
        //todo
    }
}

//Trait to implement in every game object
pub trait Room {
    /*fn to_json(&self) -> JsonValue {
        json!(self)
    }*/

    fn on_add(&mut self) {}
    fn on_remove(&mut self) {}
}


#[derive(Debug)]
pub struct RoomTimeline {
    limit: usize,
    states: Vec<String>
}

impl RoomTimeline {
    pub fn new(limit:usize) -> RoomTimeline {
        let l = if limit <= 0 { 1 } else { limit };

        RoomTimeline {  
            limit: l,
            states: vec![]
        }
    }

    pub fn set_state(&mut self, time: usize, state: String) {
        if self.states.len() >= self.limit {
            self.states.remove(0);
        }

        self.states.push(state);
    }
}
