extern crate serde;
extern crate json_patch;
extern crate nanoid;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate downcast_rs;

use downcast_rs::Downcast;
use serde::Serialize;
use std::collections::HashMap;
use std::any::Any;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};
use json_patch::diff;

pub use serde_json::{Value as JsonValue};

lazy_static! {
    static ref SERVER: Arc<RwLock<Server>> = Arc::new(RwLock::new(Server::new()));
}

//todo rename to octopus? or other thing because arena seems to be already used

pub fn get_server<F: FnOnce(&mut Server)>(handler: F) {
    let mut s = SERVER.write().unwrap();
    handler(&mut s);
}

pub struct Server {
    rooms: HashMap<String, Room>,
    states: HashMap<String, Box<State>>
}

impl Server {
    pub fn new() -> Server {
        Server {
            rooms: HashMap::new(),
            states: HashMap::new()
        }
    }

    pub fn run(&self) {
        println!("run");
    }

    pub fn add(&mut self, name: &str, state: Box<State>) -> Result<(), String> {
        if self.rooms.contains_key(name) {
            return Err(format!("The room '{}' already exists.", name));
        }

        let val = state.to_json();
        self.states.insert(name.to_string(), state);
        self.rooms.insert(name.to_string(), Room::new(name, val));

        if let (Some(s), Some(r)) = (self.states.get_mut(name), self.rooms.get_mut(name)) {
            s.on_init(r);
        }

        Ok(())
    }

    pub fn get<T: State>(&self, name: &str) -> Option<&T> {
        match self.states.get(name) {
            Some(s) => s.downcast_ref::<T>(),
            _ => None
        }
    }

    pub fn get_mut<T: State>(&mut self, name: &str) -> Option<&mut T> {
        match self.states.get_mut(name) {
            Some(s) => s.downcast_mut::<T>(),
            _ => None
        }
    }

    pub fn remove(&mut self, name: &str) -> Option<Box<State>> {
        if let (Some(mut r), Some(mut s)) = (self.rooms.remove(name), self.states.remove(name)) {
            s.on_destroy(&mut r);
            return Some(s);
        }

        None
    }
}

pub struct Room {
    id: String,
    name: String,
    connections: HashMap<String, Connection>,
    states: Vec<JsonValue>,
    state_limit: usize,
}

impl Room {
    fn new(name: &str, state: JsonValue) -> Room {
        Room::with_limit(name, state, 100)
    }

    fn with_limit(name: &str, state: JsonValue, state_limit: usize) -> Room {
        Room {
            id: nanoid::simple(),
            name: name.to_string(),
            connections: HashMap::new(),
            states: vec![state],
            state_limit: state_limit,
        }
    }

    fn add_conn(&mut self, conn: Connection) {
        //todo check collision?
        let id = conn.id.clone();
        self.connections.insert(id, conn);
        //notifiy state
    }

    fn remove_conn(&mut self, id: String) {
        self.connections.remove(&id);
        //notify state
    }

    pub fn sync_connection(&mut self, state: &State) {
        //todo
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn sync(&mut self, state: &State) {
        let current = state.to_json();
        let changes = diff(self.states.last().unwrap_or(&current), &current);

        if self.states.len() >= self.state_limit {
            self.states.remove(0);
        }

        self.states.push(current);

        //todo notifiy changes to the connections
        println!("sync {} {}", self.id, json!(changes));
    }
}


pub struct Connection {
    id: String
}

impl Connection {
    pub fn new() -> Connection {
        Connection {
            id: nanoid::simple()
        }
    }
}


pub trait State: Downcast + Send + Sync {
    fn to_json(&self) -> JsonValue;  

    fn on_init(&mut self, room: &mut Room) {
        println!("on init {}:{}", room.name(), room.id());
    }

    fn on_destroy(&mut self, room: &mut Room) {
        println!("on destroy {}:{}", room.name(), room.id());
    }

    fn on_update(&mut self, room: &mut Room) {
        println!("on update {}:{}", room.name(), room.id());
    }

    fn before_sync(&mut self, conn: &mut Connection, _room: &mut Room) -> JsonValue {
        json!({})
    }
}

impl_downcast!(State);