//! Reusable, deterministic animation graphs. No physics or renderer is required.
//! Graphs are editable text; each entity owns its clocks, parameters and triggers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Shared default locomotion graph, also used when a look has no custom graph.
pub const DEFAULT: &str = include_str!("../assets/animations/locomotion.graph");

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub name: String,
    pub clip: String,
    pub speed: f32,
    pub looping: bool,
    /// Optional clip duration override (seconds).
    pub duration: Option<f32>,
    /// Parameter, scale, minimum and maximum playback multiplier.
    pub rate: Option<(String, f32, f32, f32)>,
    /// Named gameplay markers, in clip seconds.
    pub events: Vec<(f32, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub priority: i32,
    /// Minimum real time in the source state before this transition can fire.
    pub after: f32,
    /// Comparisons joined by `&`. `finished` is supplied by the runtime.
    pub conditions: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    /// Optional phase-aware blends between named states.
    pub blends: Vec<BlendRule>,
    pub initial: String,
    pub parameters: BTreeMap<String, f32>,
    pub triggers: BTreeSet<String>,
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlendRule { pub from: String, pub to: String, pub seconds: f32 }

/// Renderer-independent description of an in-progress pose blend.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveBlend {
    pub from_clip: String,
    pub source_time: f32,
    pub elapsed: f32,
    pub seconds: f32,
}

fn number(s: &str) -> Result<f32, String> {
    s.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("invalid number `{s}`"))
}

fn comparisons(text: &str) -> Result<Vec<(&str, &str, f32)>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    text.split('&')
        .map(|term| {
            let w: Vec<_> = term.split_whitespace().collect();
            if w.len() != 3 || ![">", ">=", "<", "<=", "==", "!="].contains(&w[1]) {
                return Err(format!("use `parameter > value` joined by &: `{term}`"));
            }
            Ok((w[0], w[1], number(w[2])?))
        })
        .collect()
}

impl Graph {
    /// Parse and validate a graph with useful line errors; no partial graph escapes.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut g = Self {
            blends: vec![],
            initial: String::new(),
            parameters: BTreeMap::new(),
            triggers: BTreeSet::new(),
            states: vec![],
            transitions: vec![],
        };
        for (i, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let w: Vec<_> = line.split_whitespace().collect();
            let result = (|| -> Result<(), String> {
                match w.as_slice() {
                    ["blend", from, to, seconds] => g.blends.push(BlendRule { from: from.to_string(), to: to.to_string(), seconds: number(seconds)? }),
                    ["initial", name] => g.initial = name.to_string(),
                    ["param", name, value] => {
                        if g.parameters
                            .insert(name.to_string(), number(value)?)
                            .is_some()
                        {
                            return Err(format!("duplicate parameter `{name}`"));
                        }
                    }
                    ["trigger", name] => {
                        if !g.triggers.insert(name.to_string()) {
                            return Err(format!("duplicate trigger `{name}`"));
                        }
                    }
                    ["state", name, "clip", clip, mode, "speed", speed, rest @ ..] => {
                        if !["loop", "once"].contains(mode) {
                            return Err("state mode must be loop or once".into());
                        }
                        let mut s = State {
                            name: name.to_string(),
                            clip: clip.to_string(),
                            speed: number(speed)?,
                            looping: *mode == "loop",
                            duration: None,
                            rate: None,
                            events: vec![],
                        };
                        let mut tail = rest;
                        while !tail.is_empty() {
                            match tail {
                                ["duration", v, more @ ..] => {
                                    s.duration = Some(number(v)?);
                                    tail = more;
                                }
                                ["rate", p, scale, min, max, more @ ..] => {
                                    s.rate = Some((
                                        p.to_string(),
                                        number(scale)?,
                                        number(min)?,
                                        number(max)?,
                                    ));
                                    tail = more;
                                }
                                _ => {
                                    return Err(
                                        "expected duration SECONDS or rate PARAM SCALE MIN MAX"
                                            .into(),
                                    );
                                }
                            }
                        }
                        g.states.push(s);
                    }
                    ["event", state, time, name] => {
                        let s = g
                            .states
                            .iter_mut()
                            .find(|s| s.name == *state)
                            .ok_or("define state before its events")?;
                        s.events.push((number(time)?, name.to_string()));
                    }
                    [
                        "transition",
                        from,
                        to,
                        "priority",
                        p,
                        "after",
                        after,
                        "when",
                        conditions @ ..,
                    ] => {
                        g.transitions.push(Transition {
                            from: from.to_string(),
                            to: to.to_string(),
                            priority: p.parse().map_err(|_| "priority must be an integer")?,
                            after: number(after)?,
                            conditions: conditions.join(" "),
                        });
                    }
                    _ => {
                        return Err(
                            "expected initial, param, trigger, state, event or transition".into(),
                        );
                    }
                }
                Ok(())
            })();
            result.map_err(|e| format!("line {}: {e}", i + 1))?;
        }
        g.validate()?;
        Ok(g)
    }

    /// Validate programmatic/editor changes as strictly as file input.
    pub fn validate(&self) -> Result<(), String> {
        let identifier = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };
        let mut names = BTreeSet::new();
        for (p, v) in &self.parameters {
            if !identifier(p) || p == "finished" || self.triggers.contains(p) || !v.is_finite() {
                return Err(format!("invalid or duplicate parameter `{p}`"));
            }
        }
        for p in &self.triggers {
            if !identifier(p) || p == "finished" {
                return Err(format!("invalid trigger `{p}`"));
            }
        }
        let known = |p: &str| {
            self.parameters.contains_key(p) || self.triggers.contains(p) || p == "finished"
        };
        for s in &self.states {
            if !identifier(&s.name) || !identifier(&s.clip) || !names.insert(s.name.as_str()) {
                return Err(format!("invalid or duplicate state `{}`", s.name));
            }
            if !s.speed.is_finite()
                || s.speed <= 0.0
                || s.duration.is_some_and(|d| !d.is_finite() || d <= 0.0)
            {
                return Err(format!("{}: speed and duration must be positive", s.name));
            }
            if let Some((p, scale, min, max)) = &s.rate {
                if !self.parameters.contains_key(p)
                    || !scale.is_finite()
                    || !min.is_finite()
                    || !max.is_finite()
                    || *min < 0.0
                    || max < min
                {
                    return Err(format!("{}: invalid playback rate", s.name));
                }
            }
            if s.events
                .iter()
                .any(|(t, n)| !t.is_finite() || *t < 0.0 || !identifier(n))
            {
                return Err(format!("{}: invalid event", s.name));
            }
        }
        if !names.contains(self.initial.as_str()) {
            return Err("initial must name an existing state".into());
        }
        let mut pairs = BTreeSet::new();
        for b in &self.blends {
            if !names.contains(b.from.as_str()) || !names.contains(b.to.as_str()) || b.from == b.to || !b.seconds.is_finite() || b.seconds <= 0.0 || b.seconds > 1.5 || !pairs.insert((&b.from,&b.to)) {
                return Err(format!("invalid or duplicate blend {} -> {}", b.from, b.to));
            }
        }
        for t in &self.transitions {
            if (t.from != "*" && !names.contains(t.from.as_str())) || !names.contains(t.to.as_str())
            {
                return Err(format!("unknown transition state: {} -> {}", t.from, t.to));
            }
            if !t.after.is_finite() || t.after < 0.0 {
                return Err("transition after must be nonnegative".into());
            }
            for (p, _, _) in comparisons(&t.conditions)? {
                if !known(p) {
                    return Err(format!("unknown parameter `{p}`"));
                }
            }
        }
        Ok(())
    }

    /// Save a stable, human/AI editable representation.
    pub fn to_text(&self) -> String {
        let mut out = format!("initial {}\n", self.initial);
        for (p, v) in &self.parameters {
            out += &format!("param {p} {v}\n");
        }
        for p in &self.triggers {
            out += &format!("trigger {p}\n");
        }
        for s in &self.states {
            out += &format!(
                "state {} clip {} {} speed {}",
                s.name,
                s.clip,
                if s.looping { "loop" } else { "once" },
                s.speed
            );
            if let Some(d) = s.duration {
                out += &format!(" duration {d}");
            }
            if let Some((p, scale, min, max)) = &s.rate {
                out += &format!(" rate {p} {scale} {min} {max}");
            }
            out.push('\n');
            for (time, name) in &s.events {
                out += &format!("event {} {time} {name}\n", s.name);
            }
        }
        for t in &self.transitions {
            out += &format!(
                "transition {} {} priority {} after {} when {}\n",
                t.from, t.to, t.priority, t.after, t.conditions
            );
        }
        for b in &self.blends { out += &format!("blend {} {} {}\n", b.from, b.to, b.seconds); }
        out
    }
}

/// An event emitted while advancing one entity's graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub state: String,
    pub name: String,
}

/// Per-entity state. Clone it in game snapshots to preserve animation phase.
#[derive(Clone, Debug, PartialEq)]
pub struct Animator {
    pub blend: Option<ActiveBlend>,
    pub graph: Arc<Graph>,
    state: usize,
    pub elapsed: f32,
    pub time: f32,
    pub parameters: BTreeMap<String, f32>,
    fired: BTreeSet<String>,
    entered: bool,
    // Compile comparisons once rather than splitting strings for every entity/tick.
    conditions: Vec<Vec<(String, String, f32)>>,
}

impl Animator {
    /// Start a validated graph. Invalid programmatic graphs return an error.
    pub fn new(graph: Arc<Graph>) -> Result<Self, String> {
        graph.validate()?;
        let state = graph
            .states
            .iter()
            .position(|s| s.name == graph.initial)
            .unwrap();
        let conditions = graph
            .transitions
            .iter()
            .map(|t| {
                comparisons(&t.conditions)
                    .unwrap()
                    .into_iter()
                    .map(|(p, op, rhs)| (p.into(), op.into(), rhs))
                    .collect()
            })
            .collect();
        Ok(Self {
            blend: None,
            parameters: graph.parameters.clone(),
            graph,
            state,
            elapsed: 0.0,
            time: 0.0,
            fired: BTreeSet::new(),
            entered: true,
            conditions,
        })
    }
    pub fn state(&self) -> &State {
        &self.graph.states[self.state]
    }
    /// Set a declared parameter. Reject typos and non-finite values.
    pub fn set(&mut self, name: &str, value: f32) -> bool {
        if value.is_finite() {
            if let Some(p) = self.parameters.get_mut(name) {
                *p = value;
                return true;
            }
        }
        false
    }
    /// A one-tick request; requests are consumed even if no transition uses them.
    pub fn trigger(&mut self, name: &str) -> bool {
        if !self.graph.triggers.contains(name) {
            return false;
        }
        self.fired.insert(name.to_string());
        true
    }
    pub fn reset(&mut self) {
        *self = Self::new(self.graph.clone()).expect("validated graph");
    }
    fn duration(&self, clip_duration: &impl Fn(&str) -> f32) -> f32 {
        self.state()
            .duration
            .unwrap_or_else(|| clip_duration(&self.state().clip))
            .max(0.0001)
    }
    /// Time to sample from a rendered or skeletal clip. Once states hold their end.
    pub fn sample_time(&self, clip_duration: impl Fn(&str) -> f32) -> f32 {
        let d = self.duration(&clip_duration);
        if self.state().looping {
            self.time % d
        } else {
            self.time.min((d - 0.00001).max(0.0))
        }
    }
    /// Advance at most one transition per call. Priority wins; file order breaks ties.
    /// Markers and entry events are returned to the caller, never executed here.
    pub fn advance(&mut self, dt: f32, clip_duration: impl Fn(&str) -> f32) -> Vec<Event> {
        if !dt.is_finite() || dt < 0.0 {
            return vec![];
        }
        let mut events = vec![];
        let old = self.time;
        let was_blending = self.blend.is_some();
        if let Some(b) = &mut self.blend { b.elapsed += dt; if b.elapsed >= b.seconds { self.blend = None; } }
        // Baked destination poses advance at the authored speed during a blend;
        // parameter-driven gait rate resumes afterwards, preserving phase.
        let rate = if was_blending { 1.0 } else { self.state().rate.as_ref().map_or(1.0, |(p, k, min, max)| {
            (self.parameters[p] * k).clamp(*min, *max)
        }) };
        self.elapsed += dt;
        self.time += dt * self.state().speed * rate;
        let duration = self.duration(&clip_duration);
        for (at, name) in &self.state().events {
            // Emit once per advance even across a very large time step: bounded work.
            let hit = if self.state().looping {
                *at < duration && (((old - at) / duration).floor() < ((self.time - at) / duration).floor()
                    || (self.entered && *at == 0.0))
            } else {
                *at <= duration && ((*at > old && *at <= self.time) || (self.entered && *at == 0.0))
            };
            if hit {
                events.push(Event {
                    state: self.state().name.clone(),
                    name: name.clone(),
                });
            }
        }
        self.entered = false;
        let finished = !self.state().looping && self.time >= duration;
        let selected = self
            .graph
            .transitions
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                (t.from == "*" || t.from == self.state().name)
                    && (self.blend.is_none() || t.priority >= 50)
                    && t.to != self.state().name
                    && self.elapsed >= t.after
                    && self.conditions[*i].iter().all(|(p, op, rhs)| {
                        let v = if p == "finished" {
                            finished as u8 as f32
                        } else if self.graph.triggers.contains(p) {
                            self.fired.contains(p) as u8 as f32
                        } else {
                            self.parameters[p]
                        };
                        match op.as_str() {
                            ">" => v > *rhs,
                            ">=" => v >= *rhs,
                            "<" => v < *rhs,
                            "<=" => v <= *rhs,
                            "==" => v == *rhs,
                            "!=" => v != *rhs,
                            _ => false,
                        }
                    })
            })
            .max_by_key(|(i, t)| (t.priority, std::cmp::Reverse(*i)))
            .map(|(_, t)| t.to.clone());
        self.fired.clear();
        if let Some(to) = selected {
            let rule = self.graph.blends.iter().find(|b| b.from == self.state().name && b.to == to);
            let mut from_clip = self.state().clip.clone();
            let mut source_time = if self.state().looping { old } else { old.min((duration - 0.00001).max(0.0)) };
            if let Some(b) = &self.blend { if b.elapsed < b.seconds * 0.5 { from_clip = b.from_clip.clone(); source_time = b.source_time + b.elapsed; } }
            self.blend = rule.map(|b| ActiveBlend { from_clip, source_time, elapsed: 0.0, seconds: b.seconds });
            self.state = self.graph.states.iter().position(|s| s.name == to).unwrap();
            self.elapsed = 0.0;
            self.time = 0.0;
            self.entered = true;
            events.push(Event {
                state: to,
                name: "enter".into(),
            });
        }
        events
    }
}
