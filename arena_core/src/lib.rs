extern crate serde;
extern crate json_patch;
extern crate nanoid;
extern crate crossbeam_channel;
extern crate parking_lot;
extern crate rayon;
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
use rayon::prelude::*;

pub use serde_json::{Value as JsonValue};

type ConnId = String;
type RoomId = String;

trait Adapter {

}

struct Client {

}

#[derive(PartialEq)]
enum ClientStatus {
    Disconnected,
    Connected
}

struct InnerClient {
    id: String,
    status: ClientStatus,
    rooms: HashMap<String, JsonValue>,
}

impl InnerClient {
    fn new(id: &str) -> InnerClient {
        InnerClient {
            id: id.to_string(),
            status: ClientStatus::Disconnected,
            rooms: HashMap::new(),
        }
    }
}

pub trait ClientHandler {

}

pub struct LocalClient {
    inner: Arc<RwLock<InnerClient>>,
    server: Arena,
    conn: Connection,
}

impl LocalClient {
    pub fn new(server: Arena, manager: Box<ClientHandler>) -> LocalClient {
        let mut s = server.clone();
        
        let r_conn = s.new_conn();
        match r_conn {
            Err(e) => panic!(e),
            Ok(conn) => {
                let inner = Arc::new(RwLock::new(InnerClient::new(&conn.id)));
                inner.write().status = ClientStatus::Connected;

                let mut client = LocalClient {
                    inner: inner.clone(),
                    server: server,
                    conn: conn
                };

                println!("Client connected!!!!");
                client.run();

                client
            }
        }
    }

    fn run(&mut self) {
        let mut s = self.server.clone();
        let my_id = self.inner.read().id.clone();
        let inner = self.inner.clone();

        for evt in &self.conn.in_recv {
            println!(" # - # - # - # - # - # - # - # CLIENT EVT: {:?}", evt);
            match evt {
                ClientEvents::Msg(room_id, msg) => {
                    if msg.event == "sync" {
                        println!("\\\\ CLIENT SYNC -> {:?}", msg.data);
                    }
                },
                ClientEvents::CloseConnection(reason) => {
                    println!("Closing connection: {:?}", reason);
                    //todo change status
                    let mut c = inner.write();
                    c.status = ClientStatus::Disconnected;
                    break;
                },
                ClientEvents::JoinRoom(room_id, reason) => {
                    let mut c = inner.write();
                    c.rooms.insert(room_id.to_string(), json!({}));
                },
                _ => {}
            } 
        }
    }
}


#[derive(Debug)]
pub struct Message {
    pub event: String,
    pub data: String
}

impl Message {
    pub fn new(evt: &str, data: &str) -> Message {
        Message {
            event: evt.to_string(),
            data: data.to_string()
        }
    }
}

#[derive(Debug)]
pub enum ClientEvents {
    CloseConnection(Option<String>), //reason?
    JoinRoom(RoomId, Option<String>), //roomid, error?
    CloseRoom(RoomId, String), //roomid, reason
    Msg(RoomId, Message) //todo rename to sync?
}

#[derive(Debug)]
pub enum RoomEvents {
    OpenConnection(Connection),
    CloseConnection(ConnId),
    JoinRoom(RoomId, ConnId),
    CloseRoom(RoomId, ConnId),
    Broadcast(RoomId, Message), //room msg
    Msg(RoomId, ConnId, Message), //room, conn_id, msg
}

//todo rename to octopus, hive? or other thing because arena seems to be already used

#[derive(Debug, PartialEq)]
enum ContainerState {
    Initiating, 
    Idle,
    Destroyed,
}

#[derive(Debug)]
pub struct RoomContainer {
    room_state: ContainerState,
    kind: String, 
    room: Room,
    state: Box<State>,
    server: Arena,
}

impl RoomContainer {
    pub fn new(id: &str, kind: &str, state: Box<State>, server: Arena) -> RoomContainer {
        let json_val = state.to_json();
        RoomContainer {
            room_state: ContainerState::Initiating,
            kind: kind.to_string(),
            room: Room::new(id, kind, json_val),
            state: state,
            server: server,
        }
    }

    pub fn id(&self) -> String {
        self.room.id()
    }

    pub fn is_full(&self) -> bool {
        self.room.is_full()
    }

    pub fn add_connection(&mut self, conn: Connection) -> Result<(), String> {
        if self.room.is_full() {
            Err("Room full of connections.".to_string())
        } else {
            let id = conn.id.clone();
            self.room.add_conn(conn, &*self.state)
                .map_err(move |e| format!("Room: {}, Connection: {} -> {}", self.id(), id, e))
        }
    }

    pub fn remove_connection(&mut self, conn_id: &str) {
        let opt_conn = self.room.connections.remove(conn_id);
        match opt_conn {
            Some((c, _)) => {
                self.state.on_disconnect(conn_id, &mut self.room, &mut self.server);
                c.dispatch(ClientEvents::CloseRoom(self.room.id(), "".to_string()));
                self.sync();
            },
            _ => {}
        }
    }

    fn sync(&mut self) {
        println!("SYNC -> on container");
        self.room.sync(&*self.state, &self.server);
    }

    pub fn on_connect(&mut self, id: &str) {
        self.state.on_connect(id, &mut self.room, &mut self.server);
        if let Some((c, _)) = self.room.connections.get(id) {
            c.dispatch(ClientEvents::JoinRoom(self.room.id(), None));
        }
        self.sync();
    }

    pub fn on_init(&mut self) {
        self.state.on_init(&mut self.room, &mut self.server);
        self.room_state = ContainerState::Idle;
    }

    pub fn on_destroy(&mut self) {
        self.state.on_destroy(&mut self.room, &mut self.server);
        self.room_state = ContainerState::Destroyed;
        //todo clear connections
    }

    pub fn on_broadcast(&mut self, msg: &Message) {
        if !self.is_idle() {
            println!("Can't broadcast on container {}:{} because it's not idle yet.", self.kind, self.id());
            return; 
        }

        self.state.on_broadcast(msg, &mut self.room, &mut self.server);
        self.sync();
    }

    pub fn on_message(&mut self, conn_id: &str, msg: &Message) {
        if !self.is_idle() {
            println!("Can't send a message on container {}:{} because it's not idle yet.", self.kind, self.id());
            return; 
        }

        self.state.on_message(conn_id, msg, &mut self.room, &mut self.server);
        self.sync();
    }

    pub fn is_idle(&self) -> bool {
        self.room_state == ContainerState::Idle
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

        let list = self.list.entry(name.to_string()).or_insert(vec![]);
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
    main_room: Arc<RwLock<Option<RoomId>>>,
    list: Arc<RwLock<ContainerList>>,
    connections: Arc<RwLock<HashMap<ConnId, Connection>>>,

    in_recv: channel::Receiver<RoomEvents>,
    in_send: channel::Sender<RoomEvents>,

    client_recv: channel::Receiver<ClientEvents>,
    client_send: channel::Sender<ClientEvents>,
}

impl Arena {
    pub fn new() -> Arena {
        let (in_send, in_recv) = channel::unbounded();
        let (client_send, client_recv) = channel::unbounded();

        Arena {
            main_room: Arc::new(RwLock::new(None)),
            list: Arc::new(RwLock::new(ContainerList::new())),

            connections: Arc::new(RwLock::new(HashMap::new())),

            in_recv: in_recv,
            in_send: in_send,

            client_recv: client_recv,
            client_send: client_send,
        }
    }

    pub fn dispatch_to_client(&self, evt: ClientEvents) {
        self.client_send.send(evt);
    }

    pub fn listen_events(&mut self) -> Option<ClientEvents> {
        self.client_recv.next()
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

    pub fn send(&self, msg:RoomEvents) {
        self.in_send.send(msg);
    }

    pub fn new_conn(&mut self) -> Result<Connection, String> {
        if self.main_room.read().is_none() {
            return Err("Not found a main room.".to_string());
        }

        let mut id = nanoid::simple();
        while self.connections.read().contains_key(&id) {
            id = nanoid::simple();
        }

        let conn = Connection::with_id(&id);
        self.connections.write().insert(id, conn.clone());
        self.add_connection_to_main(conn.clone())?;

        Ok(conn)
    }

    pub fn run(&mut self) {
        use RoomEvents::*;
        
        for msg in &self.in_recv {
            println!("{:?}", msg);
            //handle messages
            match msg {
                CloseConnection(id) => {
                    self.remove_connection(&id);
                    if let Some(c) = self.connections.write().remove(&id) {
                        c.dispatch(ClientEvents::CloseConnection(Some("".to_string())));
                    }
                },
                JoinRoom(room_id, conn_id) => {
                    let opt_conn = self.connections.read().get(&conn_id)
                        .map(|conn| conn.clone());

                    match opt_conn {
                        Some(conn) => { //TODO move to conn.in_send.send(-)
                            let c = conn.clone();
                            if let Err(e) = self.add_connection_to(&room_id, conn) {
                                c.dispatch(ClientEvents::JoinRoom(room_id, Some(e)));
                            }
                        },
                        None => {
                            println!("Invalid connection id: {}", conn_id);
                        }
                    }
                },
                Broadcast(room_id, msg) => {
                    match self.list.read().get(&room_id) {
                        Some(c) => {
                            c.lock().on_broadcast(&msg);
                        },
                        None => {
                            println!("Invalid room id {} to broadcast", room_id);
                        }
                    }
                },
                Msg(room_id, conn_id, msg) => {
                    match self.list.read().get(&room_id) {
                        Some(c) => {
                            c.lock().on_message(&conn_id, &msg);
                        },
                        None => {
                            println!("Invalid room id {} to send message", room_id);
                        }
                    }
                }
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
            println!("Setting as main_room {}", id);
            *self.main_room.write() = Some(id.to_string());
            Ok(())
        }
    }

    fn add_connection_to_main(&mut self, conn: Connection) -> Result<(), String> {
        let main = self.main_room.read().clone();
        match main {
            Some(m) => self.add_connection_to(&m, conn),
            _ => Err("Not found a main room.".to_string())
        }
    }

    fn remove_connection(&mut self, conn_id: &str) {
        let list = self.list.read();
        list.containers.par_iter()
            .for_each(|(_, c)|{
                let mut container = c.lock();
                container.remove_connection(conn_id);
            });
    }

    pub fn add_connection_to(&mut self, id: &str, conn: Connection) -> Result<(), String> {
        let opt_container = self.list.read().get(id);
        match opt_container {
            Some(c) => {
                let mut container = c.lock();

                let id = conn.id.clone();
                container.add_connection(conn)?;
                container.on_connect(&id);

                Ok(())
            },
            _ => Err(format!("Room {} doesn't exists.", id))
        }
    }

    pub fn add(&mut self, name: &str, state: Box<State>) -> Result<String, String> {
        let s = self.clone();
        let id = self.list.write().add(name, state, s)?;

        println!("Added a new room {}:{}", name, id);

        let opt_container = self.list.read().get(&id);
        match opt_container {
            Some(c) => c.lock().on_init(),
            _ => ()
        }

        Ok(id)
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let container = self.list.write().remove(id)?;
        container.lock().on_destroy();

        println!("Removed room {}", id);

        Ok(())
    }

    pub fn get_ids_by_kind(&self, kind: &str) -> Option<Vec<String>> {
        self.list.read().get_ids_by_kind(kind)
    }

    pub fn get_rooms_by_kind(&self, kind: &str) -> Vec<Arc<Mutex<RoomContainer>>> {
        let list = self.list.read();
        let mut rooms = vec![];
        
        match list.get_ids_by_kind(kind) {
            Some(ids) => {
                /*let rooms:Vec<Arc<Mutex<RoomContainer>>> = ids.par_iter()
                    .filter(|id| list.containers.contains_key(*id))
                    .map(|id| list.containers.get(id).unwrap().clone())
                    .collect();

                    rooms*/
                for id in ids {
                    if let Some(container) = list.containers.get(&id) {
                        rooms.push(container.clone());
                    } else {
                        println!("Invalid container id {}", id);
                    }
                }
            },
            None => { 
                println!("Invalid kind {} requested", kind); 
                
                //vec![] 
            }
        }

        rooms
    }
}

#[derive(Debug)]
pub struct Room {
    id: String,
    kind: String,
    max_connections: Option<usize>,
    connections: HashMap<String, (Connection, Vec<JsonValue>)>,
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

    pub fn connections_len(&self) -> usize {
        self.connections.len()
    } 

    fn add_conn(&mut self, conn: Connection, state: &State) -> Result<(), String> {
        state.validate_connection(&conn)?;

        println!("connection {} added on room: {}:{}", conn.id, self.kind, self.id);
        //todo check collision?
        let id = conn.id.clone();
        self.connections.insert(id, (conn, vec![]));
        //notifiy state

        Ok(())
    }

    pub fn get_conn(&mut self, id: &str) -> Option<&mut Connection> {
        self.connections.get_mut(id).map(|(c, _)| c)
    }

    fn remove_conn(&mut self, id: &str) {
        self.connections.remove(id);
    }

    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn sync(&mut self, state: &State, server: &Arena) {
        let current = state.to_json();
        let changes = diff(self.states.last().unwrap_or(&json!({})), &current);
        
        println!("changes {:?}", changes);
        match &changes {
            json_patch::Patch(c) => {
                if c.len() == 0 {
                    return;
                }
            }
        }

        if self.states.len() >= self.state_limit {
            self.states.remove(0);
        }

        self.states.push(current);

        let limit = self.state_limit;
        let my_id = &self.id;
        let empty_json = json!({});
        self.connections.par_iter_mut()
            .for_each(move |(id, (conn, conn_states))| {
                let data = state.to_sync(id);
                let diff = diff(conn_states.last().unwrap_or(&empty_json), &data);

                match diff {
                    json_patch::Patch(changes) => {
                        if changes.len() != 0 {
                            conn_states.push(data);

                            if conn_states.len() >= limit {
                                conn_states.remove(0);
                            }

                            let json_changes = json!(changes).to_string();
                            println!("# Syncronizating {} - state {}", id, &json_changes);
                            conn.dispatch(ClientEvents::Msg(my_id.to_string(), Message::new("sync", &json_changes)));
                        }
                    }
                }
            });
    }
}


#[derive(Debug, Clone)]
pub struct Connection {
    pub id: String,
    in_recv: channel::Receiver<ClientEvents>,
    in_send: channel::Sender<ClientEvents>
}

impl Connection {
    pub fn new() -> Connection {
        Connection::with_id(&nanoid::simple())
    }

    pub fn with_id(id: &str) -> Connection {
        let (in_send, in_recv) = channel::unbounded();
        Connection {
            id: id.to_string(),
            in_recv: in_recv,
            in_send: in_send
        }
    }

    fn dispatch(&self, evt:ClientEvents) {
        self.in_send.send(evt);
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

    fn on_message(&mut self, conn_id: &str, msg: &Message, room: &mut Room, _server: &mut Arena) {
        println!("on message {}:{} conn: {} msg: {:?}", room.kind(), room.id(), conn_id, msg);
    }

    fn on_broadcast(&mut self, msg: &Message, room: &mut Room, _server: &mut Arena) {
        println!("on broadcast {}:{} msg: {:?}", room.kind(), room.id(), msg);
    }

    fn on_update(&mut self, room: &mut Room, _server: &mut Arena) {
        println!("on update {}:{}", room.kind(), room.id());
    }

    fn on_connect(&mut self, connection_id: &str, room: &mut Room, _server: &mut Arena) {
        println!("on connect [{}] {}:{}", connection_id, room.kind(), room.id());
    }

    fn on_disconnect(&mut self, connection_id: &str, room: &mut Room, _server: &mut Arena) {
        println!("on connect [{}] {}:{}", connection_id, room.kind(), room.id());
    }

    fn validate_connection(&self, _connection: &Connection) -> Result<(), String> {
        Ok(())
    }

    fn to_sync(&self, _conn_id: &str) -> JsonValue {
        self.to_json()
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