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
    kind: String, 
    room: Room,
    state: Box<State>,
    server: Arena,
}

impl RoomContainer {
    pub fn new(id: &str, kind: &str, state: Box<State>, server: Arena) -> RoomContainer {
        let json_val = state.to_json();
        RoomContainer {
            kind: kind.to_string(),
            room: Room::new(id, kind, json_val),
            state: state,
            server: server,
        }
    }

    pub fn add_connection(&mut self, conn: Connection) -> Result<(), String> {
        if self.room.is_full() {
            Err("Room full of connections.".to_string())
        } else {
            self.room.add_conn(conn);
            Ok(())
        }
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

#[derive(Debug)]
struct ContainerList {
    list: HashMap<String, Vec<String>>,
    containers: HashMap<String, Arc<Mutex<RoomContainer>>>
}

impl ContainerList {
    pub fn new() -> ContainerList {
        ContainerList {
            list: HashMap::new(),
            containers: HashMap::new()
        }
    }

    pub fn add(&mut self, name: &str, state: Box<State>, server: Arena) -> Result<String, String> {
        let mut id = nanoid::simple();
        while self.containers.contains_key(&id) {
            id = nanoid::simple();
        }

        let mut list = self.list.entry(name.to_string()).or_insert(vec![]);
        list.push(id.clone());

        self.containers.insert(id.clone(), Arc::new(Mutex::new(RoomContainer::new(&id, name, state, server))));

        Ok(id.to_owned())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.containers.contains_key(id)
    }

    pub fn get(&self, id: &str) -> Option<Arc<Mutex<RoomContainer>>> {
        self.containers.get(id).map(|c| c.clone())
    }

    pub fn get_ids_by_kind(&self, kind: &str) -> Option<Vec<String>> {
        match self.list.get(kind) {
            Some(list) => Some(list.to_vec()),
            None => None
        }
    }

    pub fn remove(&mut self, id: &str) -> Result<Arc<Mutex<RoomContainer>>, String> {
        let container = self.containers.remove(id); 
        match container {
            Some(c) => {
                let group = c.lock().kind.clone();
                if let Some(list) = self.list.get_mut(&group) {
                    if let Some(index) = list.iter().position(|v| *v == id) {
                        list.remove(index);
                    }
                }

                Ok(c)
            },
            None => Err(format!("Invalid room id '{}'", id))
        }
    }

    pub fn room_len(&self) -> usize {
        self.containers.len()
    }

    pub fn room_len_by_kind(&self, kind: &str) -> usize {
        if let Some(list) = self.list.get(kind) {
            list.len()
        } else {
            0
        }
    }
}

#[derive(Debug, Clone)]
pub struct Arena {
    main_room: Arc<RwLock<Option<String>>>,
    containers: Arc<RwLock<HashMap<String, Arc<Mutex<RoomContainer>>>>>,

    list: Arc<RwLock<ContainerList>>,

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

            list: Arc::new(RwLock::new(ContainerList::new())),

            in_recv: in_recv,
            in_send: in_send,

            bridge: ArenaBridge::new()
        }
    }

    pub fn with_main_room(name: &str, state: Box<State>) -> Arena {
        let mut s = Arena::new();
        match s.add(name, state) {
            Ok(id) => {
                if let Err(e) = s.set_main_room(&id) {
                    panic!("{}", e);
                }
            },
            _ => panic!("Something went wrong initializing the main room.")
        }

        s
    }

    pub fn room_len(&self) -> usize {
        self.list.read().room_len()
    }

    pub fn room_len_by_kind(&self, id: &str) -> usize {
        self.list.read().room_len_by_kind(id)
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

    pub fn set_main_room(&mut self, id: &str) -> Result<(), String> {
        if !self.list.read().contains(id) {
            Err(format!("Not found the room {}", id))
        } else {
            *self.main_room.write() = Some(id.to_string());
            Ok(())
        }

        /*if !self.containers.read().contains_key(name) {
            return Err(format!("Not found the room {}", name));
        }

        self.main_room.write() = Some(name.to_string());
        Ok(())*/
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

    pub fn add(&mut self, name: &str, state: Box<State>) -> Result<String, String> {
        /*{
            let mut containers = self.containers.write();
            if containers.contains_key(name) {
                return Err(format!("The room '{}' already exists.", name));
            }

            let server = self.clone();
            containers.insert(name.to_string(), Arc::new(Mutex::new(RoomContainer::new(name, state, server))));
        }*/
        let s = self.clone();
        let id = self.list.write().add(name, state, s)?;

        //clone the container reference to dispatch events without 
        //block the thread reading the container's list
        /*let opt_container = self.containers.read().get(name)
            .map(|c| c.clone());*/

        let opt_container = self.list.read().get(&id);
        match opt_container {
            Some(c) => c.lock().on_init(),
            _ => ()
        }

        Ok(id)
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        /*let c = self.containers.write().remove(name);
        match c {
            Some(c) => {
                let mut container = c.lock();
                container.on_destroy();
                Ok(())
            },
            _ => Err("Invalid name.".to_string())
        }*/

        let container = self.list.write().remove(id)?;
        container.lock().on_destroy();

        Ok(())
    }
}

#[derive(Debug)]
pub struct Room {
    id: String,
    kind: String,
    max_connections: Option<usize>,
    connections: HashMap<String, Connection>,
    states: Vec<JsonValue>,
    state_limit: usize
}

impl Room {
    fn new(id: &str, kind: &str, state: JsonValue) -> Room {
        Room::with_limit(id, kind, state, 100)
    }

    fn with_limit(id: &str, kind: &str, state: JsonValue, state_limit: usize) -> Room {
        Room {
            id: id.to_string(),
            kind: kind.to_string(),
            max_connections: None,
            connections: HashMap::new(),
            states: vec![state],
            state_limit: state_limit,
        }
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

    pub fn is_full(&self) -> bool {
        if let Some(m) = self.max_connections {
            self.connections.len() >= m
        } else {
            false
        }
    }

    fn add_conn(&mut self, conn: Connection) {
        println!("connection {} added on room: {}:{}", conn.id, self.kind, self.id);
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

    pub fn kind(&self) -> String {
        self.kind.clone()
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
        println!("on init {}:{}", room.kind(), room.id());
    }

    fn on_destroy(&mut self, room: &mut Room, _server: &mut Arena) {
        println!("on destroy {}:{}", room.kind(), room.id());
    }

    fn on_update(&mut self, room: &mut Room, _server: &mut Arena) {
        println!("on update {}:{}", room.kind(), room.id());
    }

    fn on_open_connection(&mut self, connection_id: &str, room: &mut Room, _server: &mut Arena) {
        println!("on open connection [{}] {}:{}", connection_id, room.kind(), room.id());
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