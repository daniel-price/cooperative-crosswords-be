use std::collections::HashMap;

use crate::models::api_models::{SolutionItemDto, CurrentCellDto};
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

#[derive(Message)]
#[rtype(result = "()")]
pub struct CurrentCell {
    pub x: i64,
    pub y: i64,
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

    fn broadcast_current_cell(&self, sender: WsSession, x: i64, y: i64) {
        let current_cell = CurrentCellDto {
            x,
            y,
            user: sender.user.clone(),
        };
        
        for session in self.sessions.clone().into_iter() {
            let ws_session = session.1.session;
            if ws_session.crossword == sender.crossword && ws_session.team == sender.team {
                let result = serde_json::to_string(&current_cell);
                match result {
                    Ok(message) => session.1.addr.do_send(ws_session::Message(message)),
                    Err(e) => println!("{}", e.to_string()),
                }
            }
        }
    }

    fn broadcast_all_current_positions(&self, new_session: &WsSession) {
        let mut all_positions: Vec<CurrentCellDto> = Vec::new();
        
        // Collect all current positions from other users in the same team and crossword
        for session in self.sessions.values() {
            let ws_session = &session.session;
            if ws_session.crossword == new_session.crossword 
               && ws_session.team == new_session.team 
               && ws_session.id != new_session.id {
                if let Some((x, y)) = ws_session.current_cell {
                    all_positions.push(CurrentCellDto {
                        x,
                        y,
                        user: ws_session.user.clone(),
                    });
                }
            }
        }
        
        // Send all current positions to the new client
        println!("all_positions: {:#?}", all_positions);
        if !all_positions.is_empty() {
            for position in all_positions {

            let result = serde_json::to_string(&position);
            match result {
                Ok(message) => {
                    if let Some(session) = self.sessions.get(&new_session.id) {
                        session.addr.do_send(ws_session::Message(message));
                        println!("broadcasted current positions");
                    }
                },
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

        // Broadcast all current positions to the new client
        self.broadcast_all_current_positions(&msg.session);
        
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
                println!("broadcasted moves");
            }
            Err(e) => {
                println!("{}", e.to_string())
            }
        };
    }
}

impl Handler<CurrentCell> for MoveServer {
    type Result = ();

    fn handle(&mut self, msg: CurrentCell, _: &mut Context<Self>) {
        // Update the session's current cell
        if let Some(session) = self.sessions.get_mut(&msg.sender.id) {
            session.session.current_cell = Some((msg.x, msg.y));
        }
        
        // Broadcast the current cell update to all clients in the same team and crossword
        self.broadcast_current_cell(msg.sender, msg.x, msg.y);
    }
}
