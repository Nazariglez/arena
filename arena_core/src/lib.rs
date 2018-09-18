extern crate serde;
extern crate json_patch;
extern crate nanoid;
extern crate crossbeam_channel;
extern crate parking_lot;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate downcast_rs;

use downcast_rs::Downcast;
use serde::Serialize;
use std::collections::HashMap;
use std::any::Any;
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use json_patch::diff;
use crossbeam_channel as channel;

pub use serde_json::{Value as JsonValue};

lazy_static! {
    static ref SERVER: Arc<RwLock<Arena>> = Arc::new(RwLock::new(Arena::new()));
    static ref BRIGE: ArenaBridge = ArenaBridge::new();
}


#[derive(Debug)]
enum Message {
    OpenConnection(Connection),
    CloseConection(Connection),
    MsgIn(String),
    MsgOut(String)
}

struct ArenaBridge {
    in_recv: channel::Receiver<Message>,
    in_send: channel::Sender<Message>,
    out_recv: channel::Receiver<Message>,
    out_send: channel::Sender<Message>
}

impl ArenaBridge {
    fn new() -> ArenaBridge {
        let (in_send, in_recv) = channel::bounded(0);
        let (out_send, out_recv) = channel::bounded(0);

        ArenaBridge {
            in_recv: in_recv,
            in_send: in_send,
            out_recv: out_recv,
            out_send: out_send
        }
    }

    fn send(&self, msg: Message) {
        self.in_send.send(msg);
    }

    fn listen(&self) -> &channel::Receiver<Message> {
        &self.out_recv
    }

    fn queue_msg(&self, msg: Message) {
        self.out_send.send(msg);
    }

    fn run<F: Fn(Message)>(&self, handler: F) {
        for msg in &self.in_recv {
            println!("{:?}", msg);
            handler(msg);
        }
    }
}

//todo rename to octopus? or other thing because arena seems to be already used

pub fn get<F: FnOnce(&mut Arena)>(handler: F) {
    let mut s = SERVER.write();
    handler(&mut s);
}


struct RoomContainer {
    room: Room,
    state: Box<State>,
    connections: HashMap<String, Connection>
}

impl RoomContainer {
    pub fn new(name: &str, state: Box<State>) -> RoomContainer {
        let json_val = state.to_json();
        RoomContainer {
            room: Room::new(name, json_val),
            state: state,
            connections: HashMap::new()
        }
    }

    pub fn add_connection(&mut self, conn: Connection) {
        self.room.add_conn(conn);
    }

    pub fn on_init(&mut self) {
        self.state.on_init(&mut self.room);
    }

    pub fn on_destroy(&mut self) {
        self.state.on_destroy(&mut self.room);
    }
}

#[derive(Clone)]
pub struct Arena {
    main_room: Arc<RwLock<Option<String>>>,
    containers: Arc<RwLock<HashMap<String, Mutex<RoomContainer>>>>

    //bridge: ArenaBridge
}

impl Arena {
    pub fn new() -> Arena {
        Arena {
            main_room: Arc::new(RwLock::new(None)),
            containers: Arc::new(RwLock::new(HashMap::new()))

            //bridge: ArenaBridge::new()
        }
    }

    pub fn room_len(&self) -> usize {
        self.containers.read().len()
    }

    pub fn run(&mut self) {
        
    }

    pub fn main_room(&self) -> Option<String> {
        self.main_room.read().clone()
    }

    pub fn set_main_room(&mut self, name: &str) -> Result<(), String> {
        if !self.containers.read().contains_key(name) {
            return Err(format!("Not found the room {}", name));
        }

        *self.main_room.write() = Some(name.to_string());
        Ok(())
    }

    pub fn add_connection(&mut self, conn: Connection) -> Result<(), String> {
        match self.main_room.read().clone() {
            Some(ref m) => {
                if let Some(c) = self.containers.read().get(m) {
                    let mut container = c.lock();
                    container.add_connection(conn);
                    return Ok(());
                }

                Err(format!("Main room {} doesn't exists.", m))
            },
            _ => Err("Not found a main room.".to_string())
        }
    }

    pub fn add(&mut self, name: &str, state: Box<State>) -> Result<(), String> {
        let mut containers = self.containers.write();
        if containers.contains_key(name) {
            return Err(format!("The room '{}' already exists.", name));
        }

        containers.insert(name.to_string(), Mutex::new(RoomContainer::new(name, state)));
        drop(containers);

        if let Some(c) = self.containers.read().get(name) {
            let mut container = c.lock();
            container.on_init();
        }

        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<(), String> {
        let c = self.containers.write().remove(name);
        match c {
            Some(c) => {
                let mut container = c.lock();
                container.on_destroy();
                 Ok(())
            },
            _ => Err("Invalid name.".to_string())
        }
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
        println!("connection {} added on room: {}:{}", conn.id, self.name, self.id);
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


#[derive(Debug)]
pub struct Connection {
    id: String
}

impl Connection {
    pub fn new() -> Connection {
        Connection {
            id: nanoid::simple()
        }
    }

    pub fn with_id(id: &str) -> Connection {
        Connection {
            id: id.to_string()
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


#[derive(Serialize)]
struct EmptyState;
impl State for EmptyState {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }
}