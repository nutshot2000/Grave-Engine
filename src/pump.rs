//! Stroke-driven fluid transport. Moves complete existing liquid cells:
//! a dry well, blocked outlet or missing drive never creates material.
use crate::{Materials, World, State};
use crate::physics::Physics;
use crate::material::MatId;

#[derive(Clone, Debug, PartialEq)]
pub struct Pump {
    pub liquid: MatId,
    pub drive: u64,
    /// Attachment in the drive body's local cell coordinates.
    pub anchor: (f32,f32),
    pub intake: (usize,usize,usize,usize),
    pub outlet: (usize,usize,usize,usize),
    /// Cells transported per cell of upward attachment travel.
    pub rate: f32,
    pub credit: f32,
    pub moved: u64,
    pub last_y: Option<f32>,
}
impl Pump {
    pub fn point(&self, physics:&Physics)->Option<(f32,f32)> {
        let (x,y,a)=physics.pose_of(self.drive)?;
        Some((x+self.anchor.0*a.cos()-self.anchor.1*a.sin(),y+self.anchor.0*a.sin()+self.anchor.1*a.cos()))
    }
    /// Scene attachment starts in world coordinates; snapshots already use local ones.
    pub fn bind(&mut self,physics:&Physics)->bool {
        let Some((x,y,a))=physics.pose_of(self.drive) else{return false};
        let (dx,dy)=(self.anchor.0-x,self.anchor.1-y);
        self.anchor=(dx*a.cos()+dy*a.sin(),-dx*a.sin()+dy*a.cos());
        self.last_y=self.point(physics).map(|p|p.1);true
    }
    pub fn step(&mut self,w:&mut World,m:&Materials,physics:&Physics) {
        let valid=|(x,y,width,height):(usize,usize,usize,usize)| width>0 && height>0 && width.checked_mul(height).is_some_and(|n|n<=4096) && x.checked_add(width).is_some_and(|n|n<=w.w) && y.checked_add(height).is_some_and(|n|n<=w.h);
        if !valid(self.intake) || !valid(self.outlet) || !self.rate.is_finite() || !(0.01..=64.).contains(&self.rate) || m.list.get(self.liquid as usize).is_none_or(|mat|mat.state!=State::Liquid) {
            self.credit=0.;self.last_y=None;return;
        }
        let Some((_,y))=self.point(physics) else{self.credit=0.;self.last_y=None;return};
        let lift=self.last_y.replace(y).map_or(0.,|previous|(previous-y).max(0.));
        // Reject solver tremor and bound the work even after an enormous impact.
        if lift>0.025 {self.credit=(self.credit+lift.min(3.)*self.rate).min(self.rate*20.);}
        let allowed=(self.credit.floor()as usize).min(64);
        for _ in 0..allowed {
            let (ix,iy,iw,ih)=self.intake;
            let source=(iy..iy+ih).rev().find_map(|yy|(ix..ix+iw).find_map(|xx|{
                let i=w.idx(xx,yy);(w.mat[i]==self.liquid&&!w.is_owned(i)).then_some(i)
            }));
            let (ox,oy,ow,oh)=self.outlet;
            let dest=(oy..oy+oh).rev().find_map(|yy|(ox..ox+ow).find_map(|xx|{
                let i=w.idx(xx,yy);(w.mat[i]==crate::material::AIR&&!w.is_owned(i)).then_some(i)
            }));
            let (Some(source),Some(dest))=(source,dest)else{self.credit=0.;break};
            if source==dest {self.credit=0.;break;}
            w.swap_cells(source,dest);self.credit-=1.;self.moved+=1;
        }
        debug_assert_eq!(m.get(self.liquid).state,State::Liquid);
    }
}
