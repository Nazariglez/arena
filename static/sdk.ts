enum ClientStatus {
    Connected,
    Disconected,
}

interface Message {
    room: string,
    event: string,
    data: any
}

class Client {
    status: ClientStatus = ClientStatus.Disconected;
    id: string;
    conn: WebSocket;
    rooms: {[id: string] : any} = {};

    constructor(url: string) {
        this.conn = new WebSocket(`ws://${url}`);
        
        let me = this;
        this.conn.onopen = function(evt){
            me.status = ClientStatus.Connected;
        };

        this.conn.onmessage = function(evt) {
            let data;
            try {
                data = JSON.parse(evt.data);
            }catch(e){
                console.error(e);
                return;
            }

            me._handle(data);
        };

        this.conn.onclose = function(evt) {
            me.status = ClientStatus.Disconected;
            me.onDisconnect();
        };
    }

    onDisconnect() {
        console.log("on disconnect");
    }

    _handle(msg:Message) {
        switch(msg.event) {
            case "init": 
                this.id = msg.data.id;
                break;
            case "join_room":
                if(msg.data.error) {
                    console.error(`Error joining room ${msg.room}: ${msg.data.error}`);
                } else {
                    this.rooms[msg.room] = {};
                }
                break;
            case "close_room":
                delete this.rooms[msg.room];
                break;
            default: 
                console.log(this.id, msg);
                break;
        }
    }
}



(function(){
    let conn = new Client("127.0.0.1:8088");
    (window as any).conn = conn;

})();