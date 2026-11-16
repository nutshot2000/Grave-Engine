//! Shared material collection and return-to-camp rules for games and editor Play.
use crate::{MatId, World};
use std::collections::BTreeMap;

/// One material quota, counted from actual mined cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Goal { pub material: MatId, pub target: usize, pub collected: usize }
/// A collection objective with a return point and persistent completion.
#[derive(Clone, Debug, PartialEq)]
pub struct Mission { pub home: (f32,f32), pub radius: f32, pub goals: Vec<Goal>, pub complete: bool }
impl Mission {
    /// Begin a mission with no goals; scenes or games add the quotas.
    pub fn new(home:(f32,f32),radius:f32)->Self{Self{home,radius,goals:Vec::new(),complete:false}}
    /// All configured material quotas have been met.
    pub fn ready(&self)->bool{!self.goals.is_empty()&&self.goals.iter().all(|g|g.collected>=g.target)}
    /// Award only the cells an excavation actually removed.
    pub fn collect(&mut self,removed:&BTreeMap<MatId,usize>){if self.complete{return;}for g in &mut self.goals{g.collected=g.collected.saturating_add(removed.get(&g.material).copied().unwrap_or(0));}}
    /// Finish when a living player returns with every quota.
    pub fn update(&mut self,position:(f32,f32),alive:bool){if self.ready()&&alive&&(position.0-self.home.0).hypot(position.1-self.home.1)<self.radius{self.complete=true;}}
    /// Point to the nearest needed deposit, or home after collection.
    pub fn signal(&self,world:&World,position:(f32,f32))->Option<(f32,f32)>{
        if self.ready(){return Some(self.home);}
        let id=self.goals.iter().find(|g|g.collected<g.target)?.material;
        world.mat.iter().enumerate().filter(|(_,m)|**m==id).min_by_key(|(i,_)|{let dx=(*i%world.w) as i64-position.0 as i64;let dy=(*i/world.w) as i64-position.1 as i64;dx*dx+dy*dy}).map(|(i,_)|((i%world.w) as f32,(i/world.w) as f32))
    }
}
