use std::collections::HashMap;

use crate::models::api_models::SolutionItemDto;
use crate::services::solution_service::{retrieve_and_send_solution, update_solution};
use crate::services::ws_session;
use crate::services::ws_session::WsSession;
use crate::DbPool;
use actix::prelude::*;
use actix_web::web::Data;
use uuid::Uuid;

/// New chat session is created
#[derive(Message, Debug, Clone)]
#[rtype(String)]
pub struct Connect {
    pub session: WsSession,
    pub addr: Addr<WsSession>,
}

/// Session is disconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Move {
    pub solution_items: Vec<SolutionItemDto>,
    pub sender: WsSession,
}

#[derive(Clone, Debug)]
pub struct MoveServer {
    sessions: HashMap<Uuid, Connect>,
    pool: DbPool,
}

impl MoveServer {
    pub fn new(pool: DbPool) -> MoveServer {
        MoveServer {
            sessions: HashMap::new(),
            pool,
        }
    }
}

impl MoveServer {
    fn broadcast_moves(&self, sender: WsSession, solution_items: Vec<SolutionItemDto>) {
        // Add user information to each solution item
        let solution_items_with_user: Vec<SolutionItemDto> = solution_items
            .into_iter()
            .map(|mut item| {
                item.modified_by = sender.user.clone();
                item
            })
            .collect();
        
        for session in self.sessions.clone().into_iter() {
            let ws_session = session.1.session;
            if ws_session.crossword == sender.crossword && ws_session.team == sender.team {
                let result = serde_json::to_string(&solution_items_with_user);
                match result {
                    Ok(message) => session.1.addr.do_send(ws_session::Message(message)),
                    Err(e) => println!("{}", e.to_string()),
                }
            }
        }
    }
}

impl Actor for MoveServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for MoveServer {
    type Result = String;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        println!("Someone joined: {}", msg.session.id.to_string());
        self.sessions.insert(msg.session.id, msg.clone());
        let result = futures::executor::block_on(retrieve_and_send_solution(
            Data::new(self.pool.clone()),
            msg.session.team.clone(),
            msg.session.crossword.clone(),
        ));
        return match result {
            Ok(m) => m,
            Err(e) => e.to_string(),
        };
    }
}

impl Handler<Disconnect> for MoveServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        self.sessions.remove(&msg.id);
    }
}

impl Handler<Move> for MoveServer {
    type Result = ();

    fn handle(&mut self, msg: Move, _: &mut Context<Self>) {
        let result = futures::executor::block_on(update_solution(
            Data::new(self.pool.clone()),
            msg.solution_items.clone(),
            msg.sender.user.clone(),
            msg.sender.team.clone(),
            msg.sender.crossword.clone(),
        ));
        match result {
            Ok(_) => {
                self.broadcast_moves(msg.sender.clone(), msg.solution_items.clone());
            }
            Err(e) => {
                println!("{}", e.to_string())
            }
        };
    }
}
