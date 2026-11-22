//! Client-side network connection. Simulation and socket acceptance live in the server binary.
use crate::wire::{Wire, PROTOCOL, SERVER_PACKET};
use crate::world_wire::MapState;
use serde_json::{json, Value};
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub struct Connecting {
    recv: mpsc::Receiver<Result<TcpStream, String>>,
    since: Instant,
    nick: String,
    team: String,
}
impl Connecting {
    pub fn new(host: String, port: u16, nick: String, team: String) -> Self {
        let (sender, recv) = mpsc::channel();
        thread::spawn(move || {
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map_err(|_| "Не удалось найти адрес сервера.".to_owned())
                .and_then(|addresses| {
                    for addr in addresses.take(4) {
                        if let Ok(stream) =
                            TcpStream::connect_timeout(&addr, Duration::from_secs(3))
                        {
                            return Ok(stream);
                        }
                    }
                    Err("Хост недоступен. Проверь адрес, порт и запущенный матч.".to_owned())
                });
            let _ = sender.send(result);
        });
        Self {
            recv,
            since: Instant::now(),
            nick,
            team,
        }
    }
    pub fn poll(&self) -> Result<Option<Client>, String> {
        match self.recv.try_recv() {
            Ok(Ok(stream)) => Client::from_stream(stream, &self.nick, &self.team)
                .map(Some)
                .map_err(|e| e.to_string()),
            Ok(Err(e)) => Err(e),
            Err(mpsc::TryRecvError::Empty) if self.since.elapsed() < Duration::from_secs(10) => {
                Ok(None)
            }
            _ => Err("Истекло время ожидания соединения.".to_owned()),
        }
    }
}
pub struct Client {
    wire: Wire,
    pub id: Option<u32>,
    pub developer: bool,
    last: Instant,
    last_out: Instant,
    map: MapState,
}
impl Client {
    fn from_stream(stream: TcpStream, name: &str, team: &str) -> io::Result<Self> {
        let mut wire = Wire::new(stream)?;
        wire.enqueue(Wire::packet(
            &json!({"type":"join","version":PROTOCOL,"name":name,"preferredTeam":team}),
        ))?;
        Ok(Self {
            wire,
            id: None,
            developer: false,
            last: Instant::now(),
            last_out: Instant::now(),
            map: MapState::default(),
        })
    }
    pub fn poll(&mut self) -> Result<Option<Value>, String> {
        if self.id.is_some() && self.last_out.elapsed() >= Duration::from_secs(10) {
            self.wire
                .enqueue(Wire::packet(&json!({"type":"keepalive"})))
                .map_err(|_| "Связь с сервером потеряна.".to_owned())?;
            self.last_out = Instant::now();
        }
        let packets = self
            .wire
            .frames(SERVER_PACKET)
            .map_err(|_| "Сервер закрыл соединение.".to_owned())?;
        self.wire
            .flush()
            .map_err(|_| "Связь с хостом потеряна.".to_owned())?;
        let mut latest = None;
        for mut data in packets {
            self.last = Instant::now();
            match data["type"].as_str() {
                Some("welcome") => {
                    if data["version"].as_u64() != Some(PROTOCOL) {
                        return Err("Версия сервера несовместима с клиентом.".into());
                    }
                    self.id = data["id"].as_u64().and_then(|id| u32::try_from(id).ok());
                    if self.id.is_none() {
                        return Err("Сервер прислал некорректное приветствие.".into());
                    }
                    self.developer = data["developer"].as_bool().unwrap_or(false);
                    let mut state = data["state"].take();
                    self.map.accept(&mut state)?;
                    latest = Some(state);
                }
                Some("world") if self.id.is_some() => {
                    let round = data["round"].as_u64();
                    self.map.accept(&mut data)?;
                    if latest
                        .as_ref()
                        .is_some_and(|v| v["round"].as_u64() != round)
                    {
                        latest = None;
                    }
                }
                Some("state") if self.id.is_some() => latest = Some(data["state"].take()),
                Some("error") => {
                    return Err(data["reason"].as_str().unwrap_or("Вход отклонён.").into())
                }
                _ => {}
            }
        }
        if self.last.elapsed() > Duration::from_secs(10) {
            return Err("Сервер перестал отвечать.".into());
        }
        if let Some(state) = &mut latest {
            self.map.fill(state)?;
        }
        Ok(latest)
    }
    pub fn input(&mut self, data: Value) -> io::Result<()> {
        self.command("input", data)
    }
    pub fn debug(&mut self, action: Value) -> io::Result<()> {
        if !self.developer {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Меню разработчика отключено сервером.",
            ));
        }
        self.command("debug", action)
    }
    fn command(&mut self, kind: &str, mut data: Value) -> io::Result<()> {
        let fields = data.as_object_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Команда должна быть объектом.")
        })?;
        fields.insert("type".into(), json!(kind));
        self.wire.enqueue(Wire::packet(&data))?;
        self.last_out = Instant::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    #[test]
    fn poll_applies_every_patch_in_a_burst_and_resets_at_round_change() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut client = Client::from_stream(
            TcpStream::connect(listener.local_addr().unwrap()).unwrap(),
            "A",
            "auto",
        )
        .unwrap();
        let (mut service, _) = listener.accept().unwrap();
        for packet in [
            json!({"type":"welcome","version":PROTOCOL,"id":1,"developer":true,"state":{"round":1,"propChanges":[],"builtProps":[],"groundDrops":[]}}),
            json!({"type":"world","round":1,"builtProps":[{"buildId":55}]}),
            json!({"type":"state","state":{"round":1,"tick":4}}),
            json!({"type":"world","round":1,"propChanges":[{"index":2,"stock":0}],"groundDrops":[{"id":33}]}),
            json!({"type":"state","state":{"round":1,"tick":5}}),
        ] {
            service.write_all(&Wire::packet(&packet)).unwrap();
        }
        let until = |client: &mut Client, tick: u64| {
            let end = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(state) = client.poll().unwrap() {
                    if state["tick"].as_u64() == Some(tick) {
                        break state;
                    }
                }
                assert!(Instant::now() < end);
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        let state = until(&mut client, 5);
        assert_eq!(state["builtProps"][0]["buildId"], 55);
        assert_eq!(state["propChanges"][0]["stock"], 0);
        assert!(client.developer);
        assert_eq!(state["groundDrops"][0]["id"], 33);
        for packet in [
            json!({"type":"world","round":2,"builtProps":[],"propChanges":[],"groundDrops":[]}),
            json!({"type":"state","state":{"round":2,"tick":6}}),
        ] {
            service.write_all(&Wire::packet(&packet)).unwrap();
        }
        let reset = until(&mut client, 6);
        assert_eq!(reset["builtProps"], json!([]));
        assert_eq!(reset["groundDrops"], json!([]));
        assert_eq!(reset["propChanges"], json!([]));
    }
}
