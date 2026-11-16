//! Exact mined-cell storage, data-authored tools/recipes and proximity workshops.
//! Counts are whole cells. Crafting validates the complete transaction before
//! spending anything; tool use emits actual simulated material cells.
use crate::{MatId, Materials, Sim, mining::Drill};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Supplies {
    pub tracking: bool,
    pub stock: BTreeMap<MatId, u64>,
    pub gas: BTreeMap<MatId,u64>,
    pub tools: BTreeSet<String>,
    pub equipped: String,
    pub stations: Vec<Station>,
}
impl Default for Supplies {
    fn default() -> Self {
        Self {
            tracking: false,
            stock: BTreeMap::new(), gas: BTreeMap::new(),
            tools: BTreeSet::from(["drill".into()]),
            equipped: "drill".into(),
            stations: Vec::new(),
        }
    }
}
impl Supplies {
    pub fn count(&self, id: MatId) -> u64 {
        self.stock.get(&id).copied().unwrap_or(0).saturating_add(self.gas.get(&id).copied().unwrap_or(0))
    }
    pub fn gas_total(&self)->u64 {self.gas.values().fold(0u64,|n,v|n.saturating_add(*v))}
    pub fn spend(&mut self,id:MatId,n:u64) {let solid=self.stock.entry(id).or_default();let used=(*solid).min(n);*solid-=used;if n>used{let gas=self.gas.entry(id).or_default();*gas=gas.saturating_sub(n-used);}}
    pub fn total(&self) -> u64 {
        self.stock.values().chain(self.gas.values()).fold(0u64, |n, v| n.saturating_add(*v))
    }
    pub fn nearby(&self, p: (f32, f32), kind: &str) -> bool {
        self.stations
            .iter()
            .any(|b| b.kind == kind && (b.x - p.0).hypot(b.y - p.1) <= b.radius)
    }
    pub fn equip(&mut self, id: &str) -> bool {
        if !self.tools.contains(id) {
            return false;
        }
        self.equipped = id.into();
        true
    }
    pub fn cycle(&mut self, cat: &Catalog) {
        let owned: Vec<_> = cat
            .tools
            .iter()
            .filter(|t| self.tools.contains(&t.id))
            .collect();
        if let Some(n) = owned.iter().position(|t| t.id == self.equipped) {
            self.equipped = owned[(n + 1) % owned.len()].id.clone();
        }
    }
    pub fn shift(&mut self, dx: i32, dy: i32) {
        for b in &mut self.stations {
            b.x += dx as f32;
            b.y += dy as f32;
        }
    }
}
#[derive(Clone)]
pub enum Mode {
    Drill(Drill),
    Vacuum {reach:f32,radius:f32,capacity:u64},
    Spray {
        material: MatId,
        speed: f32,
        interval: u32,
        ammo: MatId,
    },
}
#[derive(Clone)]
pub struct Tool {
    pub id: String,
    pub title: String,
    pub mode: Mode,
}
#[derive(Clone)]
pub enum Product {
    Tool(String),
    Stock(MatId, u64),
}
#[derive(Clone)]
pub struct Recipe {
    pub id: String,
    pub title: String,
    pub station: String,
    pub product: Product,
    pub costs: BTreeMap<MatId, u64>,
}
pub struct Catalog {
    pub tools: Vec<Tool>,
    pub recipes: Vec<Recipe>,
    pub problems: Vec<String>,
}
impl Catalog {
    pub fn load(m: &Materials) -> Self {
        let t = std::fs::read_to_string("assets/crafting/cave-tools.txt")
            .unwrap_or_else(|_| include_str!("../assets/crafting/cave-tools.txt").into());
        let r = std::fs::read_to_string("assets/crafting/cave-recipes.txt")
            .unwrap_or_else(|_| include_str!("../assets/crafting/cave-recipes.txt").into());
        Self::parse(&t, &r, m)
    }
    pub fn parse(tools: &str, recipes: &str, m: &Materials) -> Self {
        let mut cat = Self {
            tools: Vec::new(),
            recipes: Vec::new(),
            problems: Vec::new(),
        };
        for (n, line) in tools.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parsed = (|| -> Option<Tool> {
                let p: Vec<_> = line.split('|').map(str::trim).collect();
                if p.len() != 4 || p[0].is_empty() || cat.tools.iter().any(|t| t.id == p[0]) {
                    return None;
                }
                let w: Vec<_> = p[2].split_whitespace().collect();
                let number = |i: usize| {
                    w.get(i)?
                        .parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite() && *v > 0.0)
                };
                let mode = match w.first().copied()? {
                    "drill" if w.len() == 5 => Mode::Drill(Drill {
                        radius: number(1)?.clamp(2., 8.),
                        reach: number(2)?.clamp(8., 32.),
                        power: number(3)?.clamp(0.1, 10.),
                        grade: number(4)?.clamp(0.1, 100.),
                    }),
                    "vacuum" if w.len()==4 => Mode::Vacuum{reach:number(1)?.clamp(8.,48.),radius:number(2)?.clamp(2.,10.),capacity:w[3].parse::<u64>().ok()?.clamp(16,10000)},
                    "spray" if w.len() == 4 => Mode::Spray {
                        material: m.id(w[1])?,
                        speed: number(2)?.clamp(0.1, 4.),
                        interval: w[3].parse::<u32>().ok()?.clamp(1, 60),
                        ammo: m.id(p[3])?,
                    },
                    _ => return None,
                };
                Some(Tool {
                    id: p[0].into(),
                    title: p[1].into(),
                    mode,
                })
            })();
            match parsed {
                Some(t) => cat.tools.push(t),
                None => cat
                    .problems
                    .push(format!("tool line {} is invalid or duplicated", n + 1)),
            }
        }
        for (n, line) in recipes.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parsed = (|| -> Option<Recipe> {
                let p: Vec<_> = line.split('|').map(str::trim).collect();
                if p.len() != 5
                    || p[0].is_empty()
                    || p[2].is_empty()
                    || cat.recipes.iter().any(|r| r.id == p[0])
                {
                    return None;
                }
                let out: Vec<_> = p[3].split_whitespace().collect();
                let product = match out.as_slice() {
                    ["tool", id] if cat.tools.iter().any(|t| t.id == *id) => {
                        Product::Tool((*id).into())
                    }
                    ["stock", mat, n] => Product::Stock(
                        m.id(mat)?,
                        n.parse::<u64>().ok().filter(|n| *n > 0 && *n < 1000000)?,
                    ),
                    _ => return None,
                };
                let mut costs = BTreeMap::<MatId, u64>::new();
                for cost in p[4].split(',') {
                    let w: Vec<_> = cost.split_whitespace().collect();
                    if w.len() != 2 {
                        return None;
                    }
                    let id = m.id(w[0])?;
                    let n = w[1]
                        .parse::<u64>()
                        .ok()
                        .filter(|n| *n > 0 && *n < 1000000)?;
                    let sum = costs.get(&id).copied().unwrap_or(0).checked_add(n)?;
                    costs.insert(id, sum);
                }
                if costs.is_empty() {
                    return None;
                }
                Some(Recipe {
                    id: p[0].into(),
                    title: p[1].into(),
                    station: p[2].into(),
                    product,
                    costs,
                })
            })();
            match parsed {
                Some(r) => cat.recipes.push(r),
                None => cat
                    .problems
                    .push(format!("recipe line {} is invalid or duplicated", n + 1)),
            }
        }
        cat
    }
    pub fn tool(&self, id: &str) -> Option<&Tool> {
        self.tools.iter().find(|t| t.id == id)
    }
}
impl Recipe {
    pub fn check(&self, s: &Supplies, p: (f32, f32)) -> Result<(), String> {
        if !s.nearby(p, &self.station) {
            return Err(format!("Find a {} workshop", self.station));
        }
        if let Product::Tool(id) = &self.product {
            if s.tools.contains(id) {
                return Err("Already built".into());
            }
        }
        if self.costs.iter().any(|(id, n)| s.count(*id) < *n) {
            return Err("More materials needed".into());
        }
        if let Product::Stock(id, n) = &self.product {
            if s.count(*id).checked_add(*n).is_none() {
                return Err("Storage counter is full".into());
            }
        }
        Ok(())
    }
    pub fn craft(&self, s: &mut Supplies, p: (f32, f32)) -> Result<(), String> {
        self.check(s, p)?;
        for (&id, &n) in &self.costs {
            s.spend(id,n);
        }
        match &self.product {
            Product::Tool(id) => {
                s.tools.insert(id.clone());
                s.equipped = id.clone();
            }
            Product::Stock(id, n) => {
                *s.stock.entry(*id).or_default() += n;
            }
        }
        s.tracking = true;
        Ok(())
    }
}
impl Sim {
    /// Credit actual removals once, retaining known crystal counts from older saves.
    pub fn collect_mined(&mut self, removed: &BTreeMap<MatId, usize>) {
        if !self.supplies.tracking {
            if let Some(m) = &self.mission {
                for g in &m.goals {
                    self.supplies.stock.insert(g.material, g.collected as u64);
                }
            }
            self.supplies.tracking = true;
        }
        for (&id, &n) in removed {
            let count = self.supplies.stock.entry(id).or_default();
            *count = count.saturating_add(n as u64);
        }
        self.sync_carried_samples();
    }
    pub fn sync_carried_samples(&mut self) {
        if self.supplies.tracking {
            if let Some(m) = self.mission.as_mut().filter(|m| !m.complete) {
                for g in &mut m.goals {
                    g.collected = self.supplies.count(g.material).min(usize::MAX as u64) as usize;
                }
            }
        }
    }
    pub fn craft(&mut self, cat: &Catalog, id: &str) -> Result<(), String> {
        let p = self
            .physics
            .player_status()
            .filter(|p| !p.dead)
            .ok_or("A living player must use the workshop")?;
        let r = cat
            .recipes
            .iter()
            .find(|r| r.id == id)
            .ok_or("Unknown recipe")?;
        r.craft(&mut self.supplies, (p.x, p.y))?;
        self.sync_carried_samples();
        Ok(())
    }
    /// Named tool action for both editor and games. Actual ammo is only spent
    /// after the world accepts a projectile; dry or blocked shots cost nothing.
    pub fn use_tool(
        &mut self,
        m: &Materials,
        cat: &Catalog,
        from: (f32, f32),
        aim: (f32, f32),
    ) -> BTreeMap<MatId, usize> {
        self.use_tool_from(m, cat, from, aim, false)
    }

    pub(crate) fn use_tool_from(&mut self, m: &Materials, cat: &Catalog, from: (f32, f32),
        aim: (f32, f32), from_player: bool) -> BTreeMap<MatId, usize> {
        let empty = BTreeMap::new();
        let Some(t) = cat
            .tool(&self.supplies.equipped)
            .filter(|t| self.supplies.tools.contains(&t.id))
        else {
            return empty;
        };
        match t.mode {
            Mode::Vacuum{reach,radius,capacity} => {
                if self.world.frame%3!=0 {return empty;}
                let n=aim.0.hypot(aim.1);if n<0.01{return empty;}
                let mut removed=0;
                'scan: for step in 4..=(reach*2.) as usize {
                    let d=step as f32*0.5;let center=(from.0+aim.0/n*d,from.1+aim.1/n*d);
                    for off in -(radius as i32)..=radius as i32 {
                        if self.supplies.gas_total()>=capacity||removed>=8 {break 'scan;}
                        let (x,y)=((center.0-aim.1/n*off as f32).floor() as i32,(center.1+aim.0/n*off as f32).floor() as i32);
                        if !self.world.in_bounds(x,y){continue;}
                        let i=self.world.idx(x as usize,y as usize);let id=self.world.mat[i];
                        if id==0||self.world.is_owned(i)||m.get(id).state!=crate::material::State::Gas {continue;}
                        // Visibility from the nozzle blocks suction through rock, liquid and bodies.
                        if crate::mining::surface(&self.world,m,from,(x as f32-from.0,y as f32-from.1),(d-1.).max(4.)).is_some(){continue;}
                        let steps=((x as f32-from.0).hypot(y as f32-from.1)*2.).ceil() as usize;
                        let blocked=(1..steps).any(|k| {let t=k as f32/steps as f32;let (sx,sy)=((from.0+(x as f32-from.0)*t) as i32,(from.1+(y as f32-from.1)*t) as i32);if !self.world.in_bounds(sx,sy){return true;}let j=self.world.idx(sx as usize,sy as usize);!self.world.is_owned(j)&&m.get(self.world.mat[j]).state!=crate::material::State::Gas});
                        if blocked{continue;}
                        self.world.make_air(i);*self.supplies.gas.entry(id).or_default()+=1;removed+=1;self.supplies.tracking=true;
                    }
                }
                if removed>0 {crate::creatures::noise(&mut self.physics,from);}empty
            }
            Mode::Drill(tool) => {
                if self.mining.impact.as_ref().is_some_and(|hit| self.world.frame.wrapping_sub(hit.frame) < 6) {
                    return empty;
                }
                let removed = self.mining.strike_from(&mut self.world, m, from, aim, tool, from_player.then(|| self.physics.player_cells()));
                if self.mining.impact.as_ref().is_some_and(|hit|hit.frame==self.world.frame) {crate::creatures::noise(&mut self.physics,from);}
                self.collect_mined(&removed);
                removed
            }
            Mode::Spray {
                material,
                speed,
                interval,
                ammo,
            } => {
                if self.world.frame % interval != 0 || self.supplies.count(ammo) == 0 {
                    return empty;
                }
                let len = aim.0.hypot(aim.1);
                if len < 0.01 {
                    return empty;
                }
                let (ax, ay) = (aim.0 / len, aim.1 / len);
                let mut muzzle = None;
                let mut outside = false;
                for step in 0..=20 {
                    let d = step as f32 * 0.5;
                    let (x, y) = (from.0 + ax * d, from.1 + ay * d);
                    if x < 1.
                        || y < 1.
                        || x >= self.world.w as f32 - 1.
                        || y >= self.world.h as f32 - 1.
                    {
                        break;
                    }
                    let i = self.world.idx(x as usize, y as usize);
                    if self.world.is_owned(i) {
                        if !outside {
                            continue;
                        }
                        break;
                    }
                    outside = true;
                    if m.get(self.world.mat[i]).state != crate::material::State::Gas {
                        break;
                    }
                    muzzle = Some((x, y));
                }
                if let Some((x, y)) = muzzle {
                    let spread = ((self.world.frame as f32 * 2.399).sin()) * 0.08;
                    if self.world.spawn_flyer(
                        m,
                        material,
                        x,
                        y,
                        (ax - ay * spread) * speed,
                        (ay + ax * spread) * speed,
                    ) {
                        crate::creatures::noise(&mut self.physics,from);
                        self.supplies.spend(ammo,1);
                        self.sync_carried_samples();
                    }
                }
                empty
            }
        }
    }
}

#[derive(Default)]
pub struct Panel {
    pub open: bool,
    pub selected: usize,
    pub message: String,
    pub craft_pressed: bool,
}
pub enum Action {
    Equip(String),
    Craft(String),
}
impl Panel {
    pub fn input(&mut self, input: &crate::input::Input, count: usize) {
        if input.pressed("storage") {
            self.open = !self.open;
        }
        if self.open && count > 0 {
            if input.pressed("recipe_next") {
                self.selected = (self.selected + 1) % count;
            }
            if input.pressed("recipe_prev") {
                self.selected = (self.selected + count - 1) % count;
            }
            self.craft_pressed = input.pressed("craft");
        } else {
            self.craft_pressed = false;
        }
    }
    pub fn show(
        &mut self,
        ctx: &eframe::egui::Context,
        s: &Sim,
        m: &Materials,
        cat: &Catalog,
    ) -> Option<Action> {
        use eframe::egui::{self, Color32};
        let mut action = None;
        let mut open = self.open;
        if !open {
            return None;
        }
        let screen=ctx.viewport_rect();
        egui::Window::new("FIELD KIT  /  Inventory & workshops").open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).fixed_size(egui::vec2((screen.width()-40.).clamp(300.,790.),(screen.height()-80.).clamp(280.,630.))).max_width((screen.width()-40.).max(260.))
            .max_height((screen.height()-65.).max(250.)).collapsible(false)
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(12,23,33)).inner_margin(18.))
            .show(ctx,|ui|{
                ui.horizontal(|ui|{ui.heading("FIELD KIT");ui.separator();ui.colored_label(Color32::from_rgb(104,225,209),"WORLD PAUSED");ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{ui.small("B / L3 closes");});});
                ui.label("Every mined pixel is yours. Build equipment at a nearby workshop.");
                ui.add_space(10.);
                ui.horizontal_wrapped(|ui|{for t in &cat.tools {let owned=s.supplies.tools.contains(&t.id);let selected=s.supplies.equipped==t.id;let title=if owned {t.title.clone()}else{format!("{} · locked",t.title)};if ui.add_enabled(owned,egui::Button::new(title).selected(selected).min_size(egui::vec2(110.,34.))).clicked(){action=Some(Action::Equip(t.id.clone()));}}});
                ui.add_space(12.);ui.separator();
                let player=s.physics.player_status().filter(|p|!p.dead);let p=player.as_ref().map(|p|(p.x,p.y));
                let near:Vec<_>=s.supplies.stations.iter().filter(|b|p.is_some_and(|p|(b.x-p.0).hypot(b.y-p.1)<=b.radius)).map(|b|b.kind.replace('_'," ")).collect();
                ui.colored_label(Color32::from_rgb(255,207,130),if near.is_empty(){"WORKSHOP OFFLINE — find a mechanic or chemistry station".into()}else{format!("WORKSHOP CONNECTED / {}",near.join(" + ").to_uppercase())});
                ui.add_space(8.);
                ui.columns(2,|columns|{
                    let ui=&mut columns[0];ui.heading("Cargo");ui.small(format!("{} cells stored · exact material counts",s.supplies.total()));
                    egui::ScrollArea::vertical().id_salt("stored_materials").max_height(310.).show(ui,|ui|{
                        if s.supplies.total()==0 {ui.label("Empty — mine rock to begin collecting.");}
                        let ids:std::collections::BTreeSet<_>=s.supplies.stock.keys().chain(s.supplies.gas.keys()).copied().collect();
                        let mut ids:Vec<_>=ids.into_iter().filter(|id|s.supplies.count(*id)>0).collect();ids.sort_by_key(|id|&m.get(*id).name);
                        egui::Grid::new("cargo_counts").num_columns(2).spacing([20.,9.]).striped(true).show(ui,|ui|{for id in ids {let mat=m.get(id);let gas=s.supplies.gas.get(&id).copied().unwrap_or(0)>0;ui.colored_label(Color32::from_rgb(mat.color[0].saturating_add(35),mat.color[1].saturating_add(35),mat.color[2].saturating_add(35)),format!("{}{}",mat.name.replace('_'," "),if gas{" [tank]"}else{""}));ui.monospace(s.supplies.count(id).to_string());ui.end_row();}});
                    });
                    if let Some(t)=cat.tools.iter().find(|t|s.supplies.tools.contains(&t.id)&&matches!(t.mode,Mode::Vacuum{..})){if let Mode::Vacuum{capacity,..}=t.mode {ui.add_space(10.);ui.label(format!("Gas tank  {} / {}",s.supplies.gas_total(),capacity));ui.add(egui::ProgressBar::new(s.supplies.gas_total() as f32/capacity as f32));ui.small("Equip collector: click to draw gas in; V / RB to vent. Chemistry workshops condense fuel gas into oil.");}}
                    let ui=&mut columns[1];ui.heading("Build & refine");ui.small("Choose a recipe · green counts are ready");
                    egui::ScrollArea::vertical().id_salt("workshop_recipes").max_height(355.).show(ui,|ui|{
                        for (i,r) in cat.recipes.iter().enumerate() {
                            let check=p.ok_or_else(||"A living player is required".into()).and_then(|p|r.check(&s.supplies,p));
                            egui::Frame::group(ui.style()).fill(if self.selected==i {Color32::from_rgb(23,48,57)}else{Color32::from_rgb(17,31,43)}).inner_margin(10.).show(ui,|ui|{
                                ui.horizontal_wrapped(|ui|{if ui.selectable_label(self.selected==i,&r.title).clicked(){self.selected=i;}if ui.add_enabled(check.is_ok(),egui::Button::new("Craft")).clicked()||(self.craft_pressed&&self.selected==i&&check.is_ok()){action=Some(Action::Craft(r.id.clone()));}});
                                ui.horizontal_wrapped(|ui|{for (&id,&n) in &r.costs {ui.colored_label(if s.supplies.count(id)>=n {Color32::from_rgb(128,225,181)}else{Color32::from_rgb(245,155,120)},format!("{} {}/{}",m.get(id).name.replace('_'," "),s.supplies.count(id),n));}});
                                if let Err(reason)=check {ui.small(reason);}else{ui.colored_label(Color32::from_rgb(128,225,181),"Ready to craft");}
                            });ui.add_space(5.);
                        }
                    });
                });
                ui.separator();
                if !self.message.is_empty(){ui.colored_label(Color32::from_rgb(255,210,130),&self.message);}
                for problem in &cat.problems {ui.colored_label(Color32::LIGHT_RED,problem);}
                ui.small("↑/↓ / D-pad: recipe    Enter / Y: craft    F / LB: change equipped tool");
            });
        self.open = open;
        self.craft_pressed = false;
        action
    }
}
