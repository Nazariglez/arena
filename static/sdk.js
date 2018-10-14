var ClientStatus;
(function (ClientStatus) {
    ClientStatus[ClientStatus["Connected"] = 0] = "Connected";
    ClientStatus[ClientStatus["Disconected"] = 1] = "Disconected";
})(ClientStatus || (ClientStatus = {}));
var Client = /** @class */ (function () {
    function Client(url) {
        this.status = ClientStatus.Disconected;
        this.rooms = {};
        this.conn = new WebSocket("ws://" + url);
        var me = this;
        this.conn.onopen = function (evt) {
            me.status = ClientStatus.Connected;
        };
        this.conn.onmessage = function (evt) {
            var data;
            try {
                data = JSON.parse(evt.data);
            }
            catch (e) {
                console.error(e);
                return;
            }
            me._handle(data);
        };
        this.conn.onclose = function (evt) {
            me.status = ClientStatus.Disconected;
            me.onDisconnect();
        };
    }
    Client.prototype.onDisconnect = function () {
        console.log("on disconnect");
    };
    Client.prototype._handle = function (msg) {
        switch (msg.event) {
            case "init":
                this.id = msg.data.id;
                break;
            case "join_room":
                if (msg.data.error) {
                    console.error("Error joining room " + msg.room + ": " + msg.data.error);
                }
                else {
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
    };
    return Client;
}());
(function () {
    var conn = new Client("127.0.0.1:8088");
    window.conn = conn;
})();
