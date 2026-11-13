//! Local pose-blend banks, baked from the source rig at every source frame.
use super::*;

pub struct BlendBank {
    pub(super) key: String,
    pub(super) seconds: f32,
    pub(super) source_fps: f32,
    pub(super) target_speed: f32,
    pub(super) frames: Vec<Vec<Frame>>,
}

pub(super) fn load(dir: &Path, figure: f32, feet: f32, body_h: usize, anims: &BTreeMap<String,Anim>) -> Result<BTreeMap<(String,String),BlendBank>,String> {
    let mut banks=BTreeMap::new();
    let root=dir.join("blends");
    if !root.is_dir(){return Ok(banks);}
    for entry in std::fs::read_dir(&root).map_err(|e|e.to_string())? {
        let path=entry.map_err(|e|e.to_string())?.path();
        if !path.join("blend.txt").is_file(){continue;}
        let text=std::fs::read_to_string(path.join("blend.txt")).map_err(|e|e.to_string())?;
        let values:BTreeMap<_,_>=text.lines().filter_map(|l|l.split_once(' ')).collect();
        let get=|k:&str|values.get(k).copied().ok_or_else(||format!("{}: missing {k}",path.display()));
        let number=|k:&str| -> Result<f32,String> {get(k)?.parse::<f32>().ok().filter(|n|n.is_finite()&&*n>0.0).ok_or_else(||format!("invalid blend {k}"))};
        let from=get("from")?.to_string();let to=get("to")?.to_string();
        let source=anims.get(&from).ok_or_else(||format!("blend source clip missing: {from}"))?;
        if !anims.contains_key(&to){return Err(format!("blend destination clip missing: {to}"));}
        let seconds=number("seconds")?;let steps=number("steps")? as usize;let count=number("source_frames")? as usize;
        let fps=number("source_fps")?;let scale=number("scale")?;let target_speed=number("target_speed")?;
        if seconds>1.5 || !(2..=91).contains(&steps) || count!=source.frames.len() || (fps-source.fps).abs()>0.01 || scale>4.0 {return Err(format!("{}: incompatible blend metadata",path.display()));}
        let mut frames=Vec::with_capacity(count);
        for i in 0..count {
            let mut row=Vec::with_capacity(steps);
            for j in 0..steps {
                let pic=Picture::open(&path.join(format!("{i:03}_{j:02}.png")))?;
                if (pic.w as f32*scale-source.frames[0].w as f32).abs()>1.0 || (pic.h as f32*scale-source.frames[0].h as f32).abs()>1.0 {return Err("blend frame logical size differs from its source clip".into());}
                let mask=outline(&pic,figure/scale,feet/scale,body_h);
                row.push(Frame{w:pic.w,h:pic.h,rgba:pic.rgba,mask,scale});
            }
            frames.push(row);
        }
        let bank=BlendBank{key:format!("blend:{from}--{to}"),seconds,source_fps:fps,target_speed,frames};
        if banks.insert((from,to),bank).is_some(){return Err("duplicate pose blend bank".into());}
    }
    Ok(banks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{ActiveBlend,Graph};
    #[test]
    fn phase_selection_wraps_and_keeps_half_size_frames_at_their_logical_scale() {
        let graph=Arc::new(Graph::parse("initial idle\nstate idle clip idle loop speed 1\nstate walk clip walk loop speed 1\nblend idle walk 0.2\n").unwrap());
        let frame=|value,scale|Frame{w:2,h:2,rgba:vec![value;16],mask:vec![false;320],scale};
        let bank=BlendBank{key:"test-bank".into(),seconds:0.2,source_fps:2.0,target_speed:1.0,frames:vec![vec![frame(10,2.0),frame(11,2.0),frame(12,2.0)],vec![frame(20,2.0),frame(21,2.0),frame(22,2.0)]]};
        let look=Look{graph:graph.clone(),name:"test".into(),dir:Default::default(),figure:4.0,feet:4.0,faces_right:true,anims:[("idle".into(),Anim{frames:vec![frame(1,1.0),frame(2,1.0)],fps:2.0,looping:true}),("walk".into(),Anim{frames:vec![frame(3,1.0),frame(4,1.0)],fps:2.0,looping:true})].into(),blends:[(("idle".into(),"walk".into()),bank)].into()};
        look.validate_blends(&graph).unwrap();
        let mut blend=ActiveBlend{from_clip:"idle".into(),source_time:0.75,elapsed:0.1,seconds:0.2};
        let(f,key,index)=look.sample("walk",0.1,Some(&blend));assert_eq!((f.rgba[0],index),(21,4));assert_eq!(key,"test-bank");assert_eq!(f.scale,2.0);
        blend.source_time=1.25;
        assert_eq!(look.sample("walk",0.1,Some(&blend)).0.rgba[0],11);
        assert_eq!(look.sample("walk",0.75,None).0.rgba[0],4);
        let bad=Graph::parse(&graph.to_text().replace("walk 0.2","walk 0.3")).unwrap();
        assert!(look.validate_blends(&bad).is_err());
    }
}
