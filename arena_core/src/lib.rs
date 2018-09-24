extern crate serde;
extern crate json_patch;
extern crate nanoid;
extern crate crossbeam_channel;
extern crate parking_lot;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate downcast_rs;
#[macro_use] extern crate log;

use downcast_rs::Downcast;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use json_patch::diff;
use crossbeam_channel as channel;

pub use serde_json::{Value as JsonValue};

lazy_static! {
    static ref SERVER: Arc<RwLock<Arena>> = Arc::new(RwLock::new(Arena::new()));
    static ref BRIGE: ArenaBridge = ArenaBridge::new();
}

type ConnId = String;

#[derive(Debug)]
pub enum Message {
    OpenConnection(Connection),
    DenyConection(ConnId, String),
    CloseConnection(String),
    MsgIn(String),
    MsgOut(String)
}

#[derive(Debug, Clone)]
pub struct ArenaBridge {
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

    pub fn send(&self, msg: Message) {
        self.in_send.send(msg);
    }

    pub fn listen(&self) -> &channel::Receiver<Message> {
        &self.out_recv
    }

    pub fn queue_msg(&self, msg: Message) {
        self.out_send.send(msg);
    }

    pub fn run<F: Fn(Message)>(&self, handler: F) {
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

#[derive(Debug)]
struct RoomContainer {
    room: Room,
    state: Box<State>,
    server: Arena,
}

impl RoomContainer {
    pub fn new(name: &str, state: Box<State>, server: Arena) -> RoomContainer {
        let json_val = state.to_json();
        RoomContainer {
            room: Room::new(name, json_val),
            state: state,
            server: server,
        }
    }

    pub fn add_connection(&mut self, conn: Connection) -> Result<(), String> {
        if let Some(max) = self.room.max_connections {
            println!("PP -> {}, {}", max, self.room.connections.len());
            if self.room.connections.len() >= max {
                return Err("Room full of connections.".to_string());
            }
        }

        self.room.add_conn(conn);
        Ok(())
    }

    pub fn on_open_connection(&mut self, id: &str) {
        self.state.on_open_connection(id, &mut self.room, &mut self.server);
    }

    pub fn on_init(&mut self) {
        self.state.on_init(&mut self.room, &mut self.server);
    }

    pub fn on_destroy(&mut self) {
        self.state.on_destroy(&mut self.room, &mut self.server);
    }
}

#[derive(Debug, Clone)]
pub struct Arena {
    main_room: Arc<RwLock<Option<String>>>,
    containers: Arc<RwLock<HashMap<String, Arc<Mutex<RoomContainer>>>>>,

    in_recv: channel::Receiver<Message>,
    in_send: channel::Sender<Message>,

    pub bridge: ArenaBridge
}

impl Arena {
    pub fn new() -> Arena {
        let (in_send, in_recv) = channel::unbounded();

        Arena {
            main_room: Arc::new(RwLock::new(None)),
            containers: Arc::new(RwLock::new(HashMap::new())),

            in_recv: in_recv,
            in_send: in_send,

            bridge: ArenaBridge::new()
        }
    }

    pub fn room_len(&self) -> usize {
        self.containers.read().len()
    }

    pub fn send(&self, msg:Message) {
        self.in_send.send(msg);
    }

    pub fn run(&mut self) {
        for msg in &self.in_recv {
            println!("{:?}", msg);
            //handle messages
            match msg {
                Message::OpenConnection(conn) => {
                    if let Err(e) = self.add_connection(conn) {
                        println!("Err: {}", e); //todo deny?
                    }
                },
                _ => ()
            }
        }
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

    fn add_connection(&mut self, conn: Connection) -> Result<(), String> {
        let main = self.main_room.read().clone();
        match main {
            Some(m) => self.add_connection_to(&m, conn),
            _ => Err("Not found a main room.".to_string())
        }
    }

    pub fn add_connection_to(&mut self, room: &str, conn: Connection) -> Result<(), String> {
        let opt_container = self.containers.read().get(room)
            .map(|c| c.clone());

        match opt_container {
            Some(c) => {
                let mut container = c.lock();

                let id = conn.id.clone();
                container.add_connection(conn)?;
                container.on_open_connection(&id);

                Ok(())
            },
            _ => Err(format!("Room {} doesn't exists.", room))
        }
    }

    pub fn add(&mut self, name: &str, state: Box<State>) -> Result<(), String> {
        {
            let mut containers = self.containers.write();
            if containers.contains_key(name) {
                return Err(format!("The room '{}' already exists.", name));
            }

            let server = self.clone();
            containers.insert(name.to_string(), Arc::new(Mutex::new(RoomContainer::new(name, state, server))));
        }

        //clone the container reference to dispatch events without 
        //block the thread reading the container's list
        let opt_container = self.containers.read().get(name)
            .map(|c| c.clone());

        match opt_container {
            Some(c) => c.lock().on_init(),
            _ => ()
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

#[derive(Debug)]
pub struct Room {
    id: String,
    name: String,
    max_connections: Option<usize>,
    connections: HashMap<String, Connection>,
    states: Vec<JsonValue>,
    state_limit: usize
}

impl Room {
    fn new(name: &str, state: JsonValue) -> Room {
        Room::with_limit(name, state, 100)
    }

    pub fn set_max_connections(&mut self, amount: usize) {
        self.max_connections = Some(amount);
    }

    pub fn disable_max_connections(&mut self) {
        self.max_connections = None;
    }

    pub fn get_max_connections(&self) -> Option<usize> {
        self.max_connections
    }
 
    fn with_limit(name: &str, state: JsonValue, state_limit: usize) -> Room {
        Room {
            id: nanoid::simple(),
            name: name.to_string(),
            max_connections: None,
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

    pub fn get_conn(&mut self, id: &str) -> Option<&mut Connection> {
        self.connections.get_mut(id)
    }

    fn remove_conn(&mut self, id: String) {
        self.connections.remove(&id);
        //notify state
    }

    pub fn sync_connection(&mut self, _state: &State) {
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


#[derive(Debug, Clone)]
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


pub trait State: Downcast + Send + Sync + std::fmt::Debug {
    fn to_json(&self) -> JsonValue;  

    fn on_init(&mut self, room: &mut Room, _server: &mut Arena) {
        println!("on init {}:{}", room.name(), room.id());
    }

    fn on_destroy(&mut self, room: &mut Room, _server: &mut Arena) {
        println!("on destroy {}:{}", room.name(), room.id());
    }

    fn on_update(&mut self, room: &mut Room, _server: &mut Arena) {
        println!("on update {}:{}", room.name(), room.id());
    }

    fn on_open_connection(&mut self, connection_id: &str, room: &mut Room, _server: &mut Arena) {
        println!("on open connection [{}] {}:{}", connection_id, room.name(), room.id());
    }

    fn before_sync(&mut self, _conn: &mut Connection, _room: &mut Room, _server: &mut Arena) -> JsonValue {
        json!({})
    }
}

impl_downcast!(State);


#[derive(Debug, Serialize)]
pub struct EmptyState;
impl State for EmptyState {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }
}