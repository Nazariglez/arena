extern crate arena_core;
extern crate arena_net;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate log;
extern crate env_logger;

use arena_core::{LocalClient, Arena, State, Room, JsonValue, RoomEvents, Connection, Message, EmptyState};
use std::thread;

#[derive(Debug, Serialize)]
struct MainRoom;

impl MainRoom {
    pub fn new() -> MainRoom {
        MainRoom
    }
}

impl State for MainRoom {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }

    fn to_sync(&self, _conn_id: &str) -> JsonValue {
        self.to_json()
    }

    fn on_connect(&mut self, conn: &str, room: &mut Room, server: &mut Arena) {
        println!("on open connection [{}] {}:{}", conn, room.kind(), room.id());
        //find a waiting room or create one
        let room_id = {
            let mut id = String::from("");

            for r in server.get_rooms_by_kind("game_room") {
                let room = r.lock();
                if !room.is_full() {
                    id = room.id();
                    break;
                }
            }

            if id != "" {
                id
            } else {
                println!("All game rooms are full, creating a new one...");
                server.add("game_room", Box::new(GameRoom::new())).unwrap()
            }
        };

        //tic tac toe, create a new room for every two players
        let conn = room.get_conn(&conn);
        match conn {
            Some(c) => {
                if let Err(e) = server.add_connection_to(&room_id, c.clone()) {
                    println!("ERROR adding connection {}", e);
                }
            },
            _ => ()
        }
    }

    fn on_message(&mut self, conn_id: &str, msg: &Message, _room: &mut Room, server: &mut Arena) {
        /*match msg.event.as_ref() {
            "input" => {
                println!("input {}", msg.data); 
            },
            _ => {}
        }*/

        
    }
}

#[derive(Debug, Serialize)]
enum GameToken {
    Empty,
    Player1,
    Player2
}

#[derive(Debug, Serialize)]
enum GameState {
    Waiting,
    PlayingPlayer1,
    PlayingPlayer2,
    End
}

#[derive(Debug, Serialize)]
struct GameRoom {
    board: [[GameToken; 3]; 3],
    state: GameState,
    players: Vec<String>,
}

impl GameRoom {
    pub fn new() -> GameRoom {
        GameRoom {
            state: GameState::Waiting,

            board: [
                [ GameToken::Empty, GameToken::Empty, GameToken::Empty ],
                [ GameToken::Empty, GameToken::Empty, GameToken::Empty ],
                [ GameToken::Empty, GameToken::Empty, GameToken::Empty ],
            ],

            players: vec![]
        }
    }
}

impl State for GameRoom {
    fn to_json(&self) -> JsonValue {
        json!(self)
    }

    fn on_init(&mut self, room: &mut Room, _server: &mut Arena) {
        room.set_max_connections(2);
    }

    fn on_connect(&mut self, connection_id: &str, room: &mut Room, _server: &mut Arena) {
        self.players.push(connection_id.to_string());

        if self.players.len() == 2 {
            println!("Starting game {} with players [{} vs {}]", room.id(), self.players.get(0).unwrap(), self.players.get(1).unwrap());
            self.state = GameState::PlayingPlayer1;
        }
    }
}


pub fn main() {
    let mut server = Arena::with_main_room("main_room", Box::new(MainRoom::new()));

    let client = LocalClient::new(server.clone());

    let s = server.clone();
    let main_room = s.main_room().unwrap();
    thread::spawn(move || {
        use RoomEvents::*;

        for i in 0..1 {
            let conn = Connection::new();
            let conn_id = conn.id.clone();
            s.send(OpenConnection(conn));
            s.send(Msg(main_room.clone(), conn_id.clone(), Message::new("test", "Yeah!!")));
            s.send(Msg(main_room.clone(), conn_id, Message::new("input", "Yeah!!")));
            //s.send(Message::MsgIn(format!("i:{}", i)));
            thread::sleep_ms(100);
        }

        s.send(Broadcast(main_room, Message::new("teeeest", "nope")));
    });

    server.run();

    arena_net::run(8088);
}