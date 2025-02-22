use std::borrow::Borrow;
use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web_actors::ws;
use actix_web_actors::ws::WebsocketContext;

use crate::models::api_models::SolutionItemDto;
use crate::services::ws_server;
use crate::services::ws_server::{Move, MoveServer};
use uuid::Uuid;

/// Chat server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

#[derive(Clone, Debug)]
pub struct WsSession {
    pub id: Uuid,
    pub server_addr: Addr<MoveServer>,
    pub hb: Instant,
    pub user: String,
    pub team: String,
    pub crossword: String,
}

impl Actor for WsSession {
    type Context = WebsocketContext<WsSession>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr = ctx.address();
        self.server_addr
            .send(ws_server::Connect {
                session: self.clone(),
                addr,
            })
            .into_actor(self)
            .then(|res, _, ctx| {
                match res {
                    Ok(m) => {
                        ctx.text(m);
                    }
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // notify chat server
        self.server_addr
            .do_send(ws_server::Disconnect { id: self.id });
        Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(_)) => {
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => {
                ctx.stop();
            }
            Ok(ws::Message::Nop) => {}
            Ok(ws::Message::Text(s)) => {
                let value: Result<Vec<SolutionItemDto>, _> = serde_json::from_str(s.borrow());
                match value {
                    Ok(solution_items) => self.server_addr.do_send(Move {
                        solution_items,
                        sender: self.clone(),
                    }),
                    Err(e) => println!("Error handling text message: {:#?}", e),
                }
            }
            Err(e) => println!("Error handling stream: {:#?}", e),
        }
    }
}

impl Handler<Message> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl WsSession {
    pub fn new(addr: Addr<MoveServer>, user: String, team: String, crossword: String) -> WsSession {
        WsSession {
            id: Uuid::new_v4(),
            server_addr: addr,
            hb: Instant::now(),
            user,
            team,
            crossword,
        }
    }

    fn hb(&self, ctx: &mut WebsocketContext<WsSession>) {
        ctx.run_interval(Duration::new(1, 0), |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > Duration::new(10, 0) {
                act.server_addr
                    .do_send(ws_server::Disconnect { id: act.id });
                ctx.stop();
            }

            ctx.ping(b"Ping")
        });
    }
}
