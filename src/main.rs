/* AI evolution model http://tinyurl.com/predatorvsprey
Using this for JSON https://crates.io/crates/serde

The resources format for this program will be different from the one in python

resources
|-> default_settings.json (used when creating new simulation)
|-> simulations
    |-> simulation_name
        |-> settings.json (stuff written by human, no saved data)
        |-> simulation.json (saved simulation)
    |-> ...
*/

use std::{io, io::Read, path, env, thread, fs, io::Write, collections::HashMap, time::{Duration, Instant, SystemTime, UNIX_EPOCH}, rc::Rc, sync::{Arc, Mutex, mpsc}};
use serde::{Serialize, Deserialize};// https://stackoverflow.com/questions/60113832/rust-says-import-is-not-used-and-cant-find-imported-statements-at-the-same-time
use serde_json;
use crate::futures::Future;

// for iterating enum variants https://stackoverflow.com/questions/21371534/in-rust-is-there-a-way-to-iterate-through-the-values-of-an-enum#55056427
//use strum::IntoEnumIterator;
//use strum_macros::EnumIter;

//HTTP server: https://dzone.com/articles/from-go-to-rust-with-an-http-server
extern crate hyper;
extern crate futures;
use hyper::{Body, Response, Server, Method, StatusCode};
use hyper::service::service_fn_ok;


// Neural Net
use nn;

// 2D Vectors
use vectors2d::Vector;

// Get config
use get_config;

// Extras
use extras;

// Structs(Classes)
#[derive(Deserialize, Serialize, Debug, Clone)]
struct EntityTypeConfig {
    nn_input_size: u32,// Size of neural network input
    endurance: f64,
    speed: f64,// Max speed in units/sec
    rotation: f64,// Max rotation in degrees/sec
    nn_input: Vec<String>,// Entity input commands
    color: [u8; 3],// RGB color
    radius: u32,// Radius
    vision: u32,// Angular vision range (ex: complete vision would be 360)
    vision_range: u32,// Vision range, must be <= 1/2 of the smallest arena size for performance reasons and also because I'm lazy
    vision_bins: u32,// Number of vision bins
    health_gain: f64,// health gain per second, positive for prey, negative for predators
    special: HashMap<String, f64>,// Stuff
    population: [u32; 2],// lower- and upper-limit on population
    reproduction_time: f64// Reproduction time
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntityType {
    Predator,
    Prey
}

impl EntityType {
    pub fn to_string(&self) -> String {
        match self {
            Self::Predator => String::from("predator"),
            Self::Prey => String::from("prey")
        }
    }
    pub fn from_string(t: &str) -> Self {
        match t {
            "predator" => Self::Predator,
            "prey" => Self::Prey,
            _ => panic!("Invalid intity type")
        }
    }
    pub fn string_vector() -> Vec<String> {
        vec!["predator".to_string(), "prey".to_string()]
    }
    pub fn vector() -> Vec<Self> {
        vec![Self::Prey, Self::Predator]
    }
}

#[derive(Clone)]
struct Entity {
    nn: nn::Network,
    type_: EntityType,
    type_config: Rc<EntityTypeConfig>,// Reference counter, TODO: actualy use
    arena_size: [u32; 2],
    pos: Vector,
    orientation: f64,
    health: f64,
    energy_copy: f64,
    age: f64,
    gen: u32,
    fitness: f64,
    latest_inputs: Vec<f64>
}

#[derive(Serialize, Deserialize, Clone)]
struct EntitySave {
    nn: nn::Network,
    type_: String,
    pos: [f64; 2],
    orientation: f64,
    health: f64,
    energy_copy: f64,
    age: f64,
    gen: u32,
    fitness: f64
}

#[derive(Clone, Serialize, Deserialize)]
struct EntitySend {// for use by JS frontend
    type_: String,
    pos: [f64; 2],
    orientation: f64,
    health: f64,
    energy_copy: f64,
    age: f64,
    gen: u32,
    fitness: f64,
    latest_inputs: Vec<f64>
}

impl Entity {
    pub fn new(type_: EntityType, type_config: Rc<EntityTypeConfig>, arena_size: [u32; 2], pos: Vector, orientation: f64) -> Entity {// TODO
        // Create brain
        let input_size = type_config.nn_input_size;
        let mut nn = nn::Network::new(vec![input_size, 2], String::from("sigmoid"));
        nn.node_layers.get_mut(1).unwrap().biases[0] = 1.5;// bias all entities to move forward
        // Rest of entity
        Entity{nn, type_, type_config, arena_size, pos, orientation, health: 1.0, energy_copy: 0.0, age: 0.0, gen: 0, fitness: 0.0, latest_inputs: vec![0.0; input_size.try_into().unwrap()]}
    }
    pub fn update(&mut self, dt: f64, nn_input: Vec<f64>) {
        // store inputs
        self.latest_inputs = nn_input.clone();
        // Get movement control from NN
        let nn_result = self.nn.activation(nn_input);
        let v_linear = nn_result[0];// DO NOT CHANGE THE ORDER OF THESE
        let v_rot = nn_result[1];
        // Move
        self.orientation += v_rot * (*self.type_config).rotation * dt;
        self.orientation %= 360.0;
        self.pos.add_inplace(&Vector::from_angle_deg(self.orientation, v_linear * (*self.type_config).speed * dt));
        // Restrict position to arena size
        self.pos.mod_inplace(&Vector::from_u32_array(self.arena_size));
        // Flags
        self.age += dt;
        self.health += self.type_config.health_gain * dt;
        if self.type_ == EntityType::Prey {
            self.fitness += dt;
            self.energy_copy += dt / self.type_config.reproduction_time;
        }
        self.health = extras::extra_math::clamp(self.health, 0.0, 1.0);
    }
    pub fn see_other(&self, other: &Self) -> (u32, f64) {
        // vision bin and range ratio to other entity, ignores entity type filter
        // returns (bin, ratio)
        // NOTE: ratio, like all NN inputs ranges from -1 to 1
        // TODO: use self.edge_vision()
        let diff = toriodal_coord_comparison(self.arena_size, self.pos, other.pos);//other.pos - self.pos;
        let rel_angle = ((diff.angle_deg() - self.orientation) + 180.0).rem_euclid(360.0) - 180.0;
        let rel_dist = diff.abs();
        let vision_def = (self.type_config.vision / self.type_config.vision_bins) as f64;
        if rel_dist < self.type_config.vision_range.into() && rel_angle.abs() < (self.type_config.vision / 2).into() {
            let bin = ((rel_angle + (self.type_config.vision / 2) as f64) / vision_def).floor() as u32;// TODO: optimize (self.type_config.vision as f64)
            let ratio = extras::extra_math::clamp(rel_dist / (self.type_config.vision_range as f64), 0.0, 1.0) * -2.0 + 1.0;
            return (bin, ratio);
        }
        (0, -1.0)
    }
    /*pub fn toriodal_coord_comparison(&self) -> [bool; 8] {
        /* This function determines whether this entity could see across the edge of the simulation (it rolls over), but does not account for the orientation
        NOTE: This makes the assumption that the vision range is maller than 1/2 the smallest simulation dimension, TODO: assert this somewhere
        returns: 8-list of bools, each corresponding to the edges/corners as shown below
        3 2 1
        4   0
        5 6 7
        */
        //let mut out = [false; 8];
        /*let (x, y) = (self.pos.x, self.pos.y);
        let x_max = self.arena_size[0] as f64;
        let y_max = self.arena_size[1] as f64;
        let v_range = self.type_config.vision_range as f64;
        // edges
        // X
        if x < v_range {out[4] = true;}
        else {if x > x_max {out[0] = true;}}
        // Y
        if y < v_range {out[6] = true;}
        else {if y > y_max {out[2] = true;}}
        // corners
        if out[0] {// right
            if out[2] {// top-right
                if (Vector::from_u32_array(self.arena_size) - self.pos).abs() < v_range {
                    out[1] = true;
                    return out;
                }
            }
            else { if out[6] {// bottom-right
                if (Vector{x: x_max, y: 0.0} - self.pos).abs() < v_range {
                    out[7] = true;
                    return out;
                }
            }}
        }
        else {if out[4] {// left
            if out[2] {// top-left
                if (Vector{x: 0.0, y: y_max} - self.pos).abs() < v_range {
                    out[3] = true;
                    return out;
                }
            }
            else { if out[6] {// bottom-left
                if self.pos.abs() < v_range {
                    out[5] = true;
                    return out;
                }
            }}
        }}*/
    }*/
    pub fn offspring(&mut self, mutation: f64) -> Self {
        assert!(self.can_reproduce(), "offspring() called on entity which cannot reproduce and has type {:?}", self.type_);
        self.energy_copy -= 1.0;
        // Move self and create child pos & orientation
        let shift_vec = Vector::from_angle_deg(self.pos.angle_deg() + (90.0 * ((extras::rand_bool() as u8) as f64 * 2.0 - 1.0)), self.type_config.radius.into());
        let child_pos = self.pos + shift_vec;
        self.pos.sub_inplace(&shift_vec);
        let child_orientation = self.orientation * (extras::rand_unit() * 90.0 - 45.0);
        let mut new_nn = self.nn.clone();
        new_nn.mutate(mutation);
        // New
        Self {
            nn: new_nn,
            type_: self.type_,
            type_config: Rc::clone(&self.type_config),
            arena_size: self.arena_size,
            pos: child_pos,
            orientation: child_orientation,
            health: 1.0,
            energy_copy: 0.0,
            age: 0.0,
            gen: self.gen + 1,
            fitness: 0.0,
            latest_inputs: vec![0.0; self.type_config.nn_input_size.try_into().unwrap()]
        }
    }
    pub fn can_reproduce(&self) -> bool {
        self.energy_copy >= 1.0
    }
    pub fn eat(&mut self) {
        assert!(self.can_eat(), "Attempt to call eat() on an entity which cannot eat");
        self.fitness += 1.0;
        self.energy_copy += 1.0;
        self.health += self.type_config.special.get("eat_health_gain").expect("Could not get special key 'eat_health_gain' for Predator type config");
    }
    pub fn can_eat(&self) -> bool {
        assert_eq!(self.type_, EntityType::Predator, "Tried to call can_eat() on an entity which is not a predator");
        true// TODO: add digesgion
    }
    pub fn is_alive(&self) -> bool {
        self.health > 0.0
    }
    pub fn load_settings(&mut self, type_config: EntityTypeConfig) {
        self.type_config = Rc::new(type_config);
    }
    // for use by JS frontend
    pub fn send(&self) -> EntitySend {
        EntitySend {
            type_: self.type_.to_string(),
            pos: [self.pos.x, self.pos.y],
            orientation: self.orientation,
            health: self.health,
            energy_copy: self.energy_copy,
            age: self.age,
            gen: self.gen,
            fitness: self.fitness,
            latest_inputs: self.latest_inputs.clone()
        }
    }
    // return serializable type
    pub fn save(&self) -> EntitySave {
        EntitySave {
            nn: self.nn.clone(),
            type_: self.type_.to_string(),
            pos: [self.pos.x, self.pos.y],
            orientation: self.orientation,
            health: self.health,
            energy_copy: self.energy_copy,
            age: self.age,
            gen: self.gen,
            fitness: self.fitness
        }
    }
    // opposite of `save`
    pub fn load(es: EntitySave, type_config: Rc<EntityTypeConfig>, arena_size: [u32; 2]) -> Self {
        let input_size = type_config.nn_input_size;
        Entity{
            nn: es.nn,
            type_: EntityType::from_string(&es.type_),
            type_config: type_config,// Reference, to save memory
            arena_size,
            pos: Vector{x: es.pos[0], y: es.pos[1]},
            orientation: es.orientation,
            health: es.health,
            energy_copy: es.energy_copy,
            age: es.age,
            gen: es.gen,
            fitness: es.fitness,
            latest_inputs: vec![0.0; input_size.try_into().unwrap()]
        }
    }
    pub fn random_pos(arena_size: [u32; 2]) -> Vector {
        let x = extras::rand_unit();
        Vector{x: (arena_size[0] as f64) * x, y: (arena_size[1] as f64) * extras::rand_unit()}
    }
}

pub struct Simulation {
    size: [u32; 2],
    entities: HashMap<u64, Entity>,
    name: String,
    t: Duration,
    t_saved: u64,// unix timestamp for keeping track of when it was saved
    next_unused_entity_id: u64,
    settings: Settings
}

#[derive(Serialize, Deserialize)]
pub struct SimulationSave {
    size: [u32; 2],
    entities: HashMap<u64, EntitySave>,
    t: f64,
    t_saved: u64,
    next_unused_entity_id: u64
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SimulationSend {// for use by JS frontend
    size: [u32; 2],
    entities: HashMap<u64, EntitySend>,
    t: f64,
    t_saved: u64,
    fps: f64
}

enum ServerThreadMessage {
    Save,
    ReloadSettings/*,
    Stop,
    Start,
    Step*/
}

impl Simulation {
    pub fn new(size: [u32; 2], n_entity_types: HashMap<EntityType, u32>, name: String) -> Self {
        // Load settings
        let settings = resources::default_settings();
        // Compile entities
        let mut entities = HashMap::<u64, Entity>::new();
        let mut n_entities = 0;
        for (type_, count) in n_entity_types.iter() {
            for _ in 0..*count {
                entities.insert(
                    n_entities.try_into().unwrap(),
                    Entity::new(
                        *type_,
                        Rc::new(settings.entity_types.get(&type_.to_string()).expect("Missing key in entity type counts").clone()),
                        size,
                        Entity::random_pos(size),
                        extras::rand_unit() * 360.0
                    )
                );
                n_entities += 1;// Don't forget to do this
            }
        }
        Simulation {
            size,
            entities,
            name,
            t: Duration::ZERO,
            t_saved: 0,
            next_unused_entity_id: n_entities,
            settings: resources::default_settings()
        }
    }
    pub fn main_loop(&mut self) {
        // Create mutex to store state of self https://doc.rust-lang.org/stable/book/ch16-03-shared-state.html
        let self_send_mutex = Arc::new(Mutex::new(self.send(0.0)));
        // sender/reciever for messages from server thread
        let (tx, rx) = mpsc::channel::<ServerThreadMessage>();
        // Start HTTP server
        let server_thread_handle = self.spawn_server(Arc::clone(&self_send_mutex), tx);
        // init stuff
        let mut prev_t = Instant::now();
        let mut dt: Duration = Duration::new(0, 0);
        let mut dt_f64: f64 = 0.0;
        let mut fps: f64 = 0.0;
        // Main blocking loop, I want this to run as FAST as possible
        loop {
            // 1: Timing
            dt = prev_t.elapsed();
            prev_t = Instant::now();
            self.t += dt;
            dt_f64 = extras::get_secs(dt);
            fps = 1.0 / dt_f64;
            // 2: Update send mutex, done inside block so lock is released when done
            {
                *(self_send_mutex.lock().unwrap()) = self.send(fps);
            }
            // Recieve messages from server thread
            loop {
                match rx.try_recv() {// https://doc.rust-lang.org/stable/std/sync/mpsc/struct.Receiver.html
                    Ok(value) => {
                        match value {
                            ServerThreadMessage::Save => resources::save_sim(self),
                            ServerThreadMessage::ReloadSettings => self.load_settings()
                        }
                    },
                    Err(error) => {
                        match error {
                            mpsc::TryRecvError::Empty => break,
                            mpsc::TryRecvError::Disconnected => panic!("Server thread message queue has been disconnected")
                        }
                    }
                }
            }
            // 4: Update entities
            let mut IDs =  Vec::new();
            for (id, _) in self.entities.iter() {
                IDs.push(*id);
            }
            for id in IDs.iter() {
                let nn_inputs = self.entity_nn_input(&self.entities[id]);// Immutable
                self.entities.get_mut(id).unwrap().update(dt_f64, nn_inputs);// Mutable
            }
            // 5: Delete dead entitie sand spawn new ones
            self.spawn_and_death_rules();
        }
        server_thread_handle.join();
    }
    fn spawn_server(&mut self, send_mutex: Arc<Mutex<SimulationSend>>, tx: mpsc::Sender<ServerThreadMessage>) -> thread::JoinHandle<()> {
        // is single-threaded for now, TODO: thread pool
        let sim_name = self.name.clone();
        let router = move || {
            let mutex_copy = send_mutex.clone();
            let sim_name_copy = sim_name.clone();
            let tx = tx.clone();
            //let self_name_copy = self.name.clone();
            service_fn_ok( move |req| {
                match(req.method(), req.uri().path()) {
                    (&Method::GET, "/sim.json") => {
                        let res_body = serde_json::to_string(
                            &*(*(mutex_copy.clone())).lock().unwrap()
                        ).unwrap();
                        let mut res = Response::new(Body::from(res_body));
                        *res.status_mut() = StatusCode::from_u16(200).expect("200 supposedly invalid HTTP code");
                        res
                    },
                    (&Method::GET, "/settings.json") => {
                        let res_body = serde_json::to_string(
                            &resources::load_settings(&sim_name_copy)
                        ).unwrap();
                        let mut res = Response::new(Body::from(res_body));
                        *res.status_mut() = StatusCode::from_u16(200).expect("200 supposedly invalid HTTP code");
                        res
                    },
                    (&Method::GET, "/reload_settings") => {
                        // Send message
                        tx.send(ServerThreadMessage::ReloadSettings).unwrap();
                        // Response
                        let mut res = Response::new(Body::from("Reloaded settings"));
                        *res.status_mut() = StatusCode::from_u16(200).expect("200 supposedly invalid HTTP code");
                        res
                    },
                    (&Method::GET, "/save") => {
                        // Send message
                        tx.send(ServerThreadMessage::Save).unwrap();
                        // Response
                        let mut res = Response::new(Body::from("Saved"));
                        *res.status_mut() = StatusCode::from_u16(200).expect("200 supposedly invalid HTTP code");
                        res
                    },
                    (&Method::GET, file_path) => {
                        let (res_body, code) = resources::http_query_file(file_path.to_owned());
                        let mut res = Response::new(Body::from(res_body));
                        *res.status_mut() = StatusCode::from_u16(code).expect("Invalid HTTP code returned from resources::http_query_file()");
                        res
                    },
                    (_, _) => {
                        let mut res = Response::new(Body::from("not found"));
                        *res.status_mut() = StatusCode::NOT_FOUND;
                        res
                    }
                }
            })
        };
        let addr = format!("{}:{}", &get_config::get_ip(), &get_config::get_port("ai-simulator")).parse().unwrap();
        let server = Server::bind(&addr).serve(router);
        thread::spawn(
            || hyper::rt::run(
                server.map_err(
                    |e| {
                        eprintln!("server error: {}", e);
                    }
                )
            )
        )
    }
    fn spawn_and_death_rules(&mut self) {
        let entity_counts = self.entity_type_counts();
        let prey_predator_collisions = self.prey_predator_collisions();
        for e_type in [EntityType::Prey, EntityType::Predator] {// order is important, DO NOT CHANGE
            let e_fittest_ids = self.fittest(e_type);
            let mut e_count = *entity_counts.get(&e_type.to_string()).unwrap();
            let [min_pop, max_pop] = self.settings.entity_types.get(&e_type.to_string()).unwrap().population;
            // death
            if e_count > min_pop {
                for id in e_fittest_ids.iter().rev() {
                    let e = (*self.entities.get(id).unwrap()).clone();
                    let mut die = false;
                    if e.type_ == EntityType::Prey && prey_predator_collisions.contains_key(id) {// prey on the collision dict keys
                        die = true;
                        self.entities.get_mut(prey_predator_collisions.get(&id).unwrap()).unwrap().eat();
                    }
                    if die || !e.is_alive() {// entity not alive or is a prey on the eaten list
                        self.entities.remove(id);
                        e_count -= 1;
                        //println!("Killed entity of type: {}, pop: {}, min_pop: {}", e_type.to_string(), e_count, min_pop);
                    }
                    if e_count <= min_pop {
                        break;
                    }
                }
            }
            // spawn, carefull to handle errors when entity ID no longer exists
            if e_count < max_pop {
                for id in e_fittest_ids.iter() {
                    if let Some(e) = self.entities.get(id) {// continue if id no longer valid
                        if e.can_reproduce() {
                            let new_id = self.get_new_entity_id();
                            let new_entity = self.entities.get_mut(id).unwrap().offspring(self.settings.mutation);
                            self.entities.insert(new_id, new_entity);
                            e_count += 1;
                        }
                        if e_count >= max_pop {
                            break;
                        }
                    }
                }
            }
        }
    }
    fn prey_predator_collisions(&mut self) -> HashMap<u64, u64> {
        // gets dict of prey- and predator-IDs
        // NOTE: this function calls .eat() on predators, but does not do anything to the prey
        let mut pairs = HashMap::<u64, u64>::new();
        let mut prey_eaten = Vec::<u64>::new();
        for (id0, e0) in self.entities.iter() {
            for (id1, e1) in self.entities.iter() {
                if id0 > id1 && (e0.type_ != e1.type_) && (e0.pos - e1.pos).abs() < (e0.type_config.radius + e1.type_config.radius).into() {// don't double-check pairs or the same entity, this makes the assumption that there are only 2 entity types
                    // get info
                    let (prey_id, predator_id, predator) = {
                        if e0.type_ == EntityType::Prey {
                            (*id0, *id1, e1)
                        }
                        else {
                            (*id1, *id0, e0)
                        }
                    };
                    // check if predator can eat and prey not already eaten
                    if predator.can_eat() && !prey_eaten.contains(&prey_id) {
                        pairs.insert(prey_id, predator_id);
                        prey_eaten.push(prey_id);
                    }
                }
            }
        }
        pairs
    }
    fn fittest(&self, type_: EntityType) -> Vec<u64> {
        // Create vector of entities of `type_` from most to least "fit"
        let mut out = self.get_entity_ids_of_type(type_);
        out.sort_by_key(|id| ((0.0 - self.entities.get(id).unwrap().fitness) * 1000000.0/* floats don't work for sorting*/) as i64);
        out
    }
    fn get_entity_ids_of_type(&self, type_: EntityType) -> Vec<u64> {
        // Create vector
        let mut out = Vec::new();
        // Get list of entities that are `type_`
        for (id, entity) in self.entities.iter() {
            if entity.type_ == type_ {
                out.push(*id);
            }
        }
        out
    }
    fn get_new_entity_id(&mut self) -> u64 {
        let tmp = self.next_unused_entity_id;
        self.next_unused_entity_id += 1;
        tmp + 1
    }
    fn entity_nn_input(&self, entity: &Entity) -> Vec<f64> {
        let mut out = Vec::new();
        for input_type in entity.type_config.nn_input.iter() {
            let args: Vec<&str> = input_type.split(" ").collect();
            match args[0] {
                "vision" => out.extend(self.entity_vision_bins(entity, args[1])),
                "c" => out.push(args[1].parse::<f64>().expect("Invalid float NN input constant in settings")),
                &_ => panic!("Invalid entity vision command (this error originates in Simulation.entity_nn_input())")
            }
        }
        out
    }
    fn entity_vision_bins(&self, entity: &Entity, type_filter: &str) -> Vec<f64> {
        let mut out = vec![-1.0; entity.type_config.vision_bins.try_into().unwrap()];
        let type_filter = EntityType::from_string(type_filter);
        for (_, other) in self.entities.iter() {
            if other.type_ == type_filter {
                let (bin, ratio) = entity.see_other(other);
                let bin_usize = usize::try_from(bin).unwrap();
                if out[bin_usize] < ratio {// Update if its the closest other entity in that bin
                    out[bin_usize] = ratio;
                }
            }
        }
        out
    }
    fn load_settings(&mut self) {
        // Load settings
        self.settings = resources::load_settings(&self.name);
        for (_, e) in self.entities.iter_mut() {
            let type_str = &e.type_.to_string();
            e.load_settings(self.settings.entity_types.get(type_str).expect(&format!("Could not find settings for entity type: {}", type_str)).clone());// TODO: this copies the entity type config, this could be memory-optimized
        }
    }
    fn entity_type_counts(&self) -> HashMap<String, u32> {
        // init dict to 0s
        let mut counts = HashMap::<String, u32>::new();
        for e_type in EntityType::string_vector().iter() {
            counts.insert(e_type.clone(), 0 as u32);
        }
        // Count
        for (_, entity) in self.entities.iter() {
            *counts.get_mut(&entity.type_.to_string()).unwrap() += 1;// https://stackoverflow.com/questions/30414424/how-can-i-update-a-value-in-a-mutable-hashmap
        }
        counts
    }
    fn send(&self, fps: f64) -> SimulationSend {// for use by JS frontend
        // create serializable entities
        let mut save_entities = HashMap::<u64, EntitySend>::new();
        for (i, entity) in self.entities.iter() {
            save_entities.insert(*i, entity.send());
        }
        SimulationSend {
            size: self.size,
            entities: save_entities,
            t: extras::get_secs(self.t),
            t_saved: self.t_saved,//extras::get_unix_timestamp(self.t_saved),// I have given up trying to get the epoch timestamp in this language
            fps
        }
    }
    pub fn save(&mut self) -> SimulationSave {
		// update saved time
		self.t_saved = extras::get_unix_ts_secs() as u64;
        // create serializable entities
        let mut save_entities = HashMap::<u64, EntitySave>::new();
        for (i, entity) in self.entities.iter() {
            save_entities.insert(*i, entity.save());
        }
        SimulationSave {
            size: self.size,
            entities: save_entities,
            t: extras::get_secs(self.t),
            t_saved: self.t_saved,
            next_unused_entity_id: self.next_unused_entity_id
        }
    }
    pub fn load(saved: SimulationSave, name: String) -> Self {
        // Settings
        let settings = resources::load_settings(&name);
        // Entities
        let mut entities = HashMap::new();
        // iterate through entity save objects to create entities
        for (i, entity_save) in saved.entities.iter() {
            entities.insert(
                *i,
                Entity::load(
                    entity_save.clone(),
                    Rc::new(settings.entity_types.get(&entity_save.type_).expect(&format!("Invalid entity type \"{}\"", &entity_save.type_)).clone()),
                    saved.size
                )
            );
        }
        Simulation {
            size: saved.size,
            entities: entities,
            name,
            t: extras::from_secs(saved.t), 
            t_saved: saved.t_saved,
            next_unused_entity_id: saved.next_unused_entity_id,
            settings
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Settings {
    // represents data loaded from settings.json
    entity_types: HashMap<String, EntityTypeConfig>,
    edge_kill: bool,
    mutation: f64,
    inactivity_reset_time: f64,
    enable_dynamic_settings: bool
}

pub mod resources {
    use super::*;
    static RESOURCE_DIR: &str = "resources/";
    static HTTP_DIR: &str = "resources/http";
    static SIMULATIONS_DIR: &str = "resources/simulations/";
    pub fn load_settings(name: &str) -> Settings {
        load_settings_filename(&(SIMULATIONS_DIR.to_owned() + name + "/settings.json"))
    }
    pub fn default_settings() -> Settings {
        load_settings_filename("resources/default_settings.json")
    }
    pub fn load_settings_filename(filename: &str) -> Settings {
        let contents = fs::read_to_string(filename).expect(&format!("Should have been able to read text file {}", filename));
        // https://crates.io/crates/serde_json
        let settings: Settings = serde_json::from_str(&contents).expect("Could not deserialize settings file into `Settings` type");
        settings
    }
    pub fn load_sim(name: &str) -> Simulation {
        let contents = fs::read_to_string(SIMULATIONS_DIR.to_owned() + name + "/simulation.json").unwrap();
        let sim_save: SimulationSave = serde_json::from_str(&contents).unwrap();
        Simulation::load(sim_save, String::from(name))
    }
    pub fn save_sim(sim: &mut Simulation) {
        let sim_save = sim.save();
        // Check if directory exists, otherwise create it
        let dir_name = SIMULATIONS_DIR.to_owned() + &sim.name;
        fs::create_dir_all(dir_name.clone()).expect(&format!("Could not create/check existence of directory: {dir_name}"));// https://stackoverflow.com/questions/48053933/how-to-check-if-a-directory-exists-and-create-a-new-one-if-it-doesnt
        // Open file
        let file_name = dir_name + "/simulation.json";
        let mut file = fs::File::create(file_name.clone()).expect(&format!("Could not write to file {file_name}"));
        // Save
        let contents = serde_json::to_string(&sim_save).unwrap();
        file.write((&contents).as_bytes());
    }
    pub fn copy_default_settings(name: &str) {
        let file_path = SIMULATIONS_DIR.to_owned() + name + "/settings.json";
        // create file
        fs::File::create(file_path.clone()).expect(&format!("Could not create file {file_path}"));
        // copy default_settings.json to /simulations/`name`/settings.json
        fs::copy(RESOURCE_DIR.to_owned() + "/default_settings.json", file_path).expect("Could not copy default settings");
    }
    pub fn http_query_file(http_file_path: String) -> (Vec<u8>, u16) {
        let mut file_path = HTTP_DIR.to_owned() + &http_file_path;
        if path::Path::new(&file_path).exists() {
            // https://www.dotnetperls.com/read-bytes-rust
            if path::Path::new(&file_path).is_dir() {
                file_path.push_str("/index.html");
            }
            let f = match fs::File::open(&file_path) {
                Ok(f) => f,
                Err(_) => panic!("Could not open file '{}'", file_path)
            };
            let mut reader = io::BufReader::new(f);
            let mut buffer = Vec::new();
            // Read file into vector
            reader.read_to_end(&mut buffer).expect(&format!("Could not load file {file_path} into Vec<u8>"));
            (buffer, 200)
        }
        else {
            (b"Not Found".to_vec(), 404)
        }
    }
}

fn toriodal_coord_comparison(arena_size: [u32; 2], p1: Vector, p2: Vector) -> Vector {
    // gets the angle and length of closest straight path from p1 to p2
    // returns: difference vector
    let mut out = [0.0; 2];
    for i in 0..2 {
        let (v1, v2, limit_int) = ([p1.x, p1.y][i], [p2.x, p2.y][i], arena_size[i]);
        let d = v2 - v1;
        let limit = limit_int as f64;
        out[i] = if d.abs() > limit / 2.0 {// shorter path would be across edge
            if d > 0.0 {
                - (limit - d)
            }
            else {
                limit + d
            }
        }
        else {
            d
        }
    }
    Vector{x: out[0], y: out[1]}
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
	#[test]
	fn toriodal_coord_comparison_normal() {
		assert_eq!(toriodal_coord_comparison([100, 100], Vector{x: 1.0, y: 2.0}, Vector{x: 3.0, y: 6.0}), Vector{x: 2.0, y: 4.0});
	}
	#[test]
	fn toriodal_coord_comparison_rollover_forward() {
		assert_eq!(toriodal_coord_comparison([100, 100], Vector{x: 99.0, y: 98.0}, Vector{x: 1.0, y: 2.0}), Vector{x: 2.0, y: 4.0});
	}
	#[test]
	fn toriodal_coord_comparison_rollover_backward() {
		assert_eq!(toriodal_coord_comparison([100, 100], Vector{x: 1.0, y: 2.0}, Vector{x: 99.0, y: 98.0}), Vector{x: -2.0, y: -4.0});
	}
	#[test]
	fn toriodal_coord_comparison_rollover_specific_case() {
		assert_eq!(toriodal_coord_comparison([1000, 1000], Vector{x: 500.0, y: 900.0}, Vector{x: 500.0, y: 100.0}), Vector{x: 0.0, y: 200.0});
	}
}

fn main() {
    println!("AI Evolution simulation using Neural Nets");
    let args: Vec<String> = env::args().collect();
    println!("Arguments: {:?}", args);
    if args.len() < 2 {
        panic!("Not enough program arguments");
    }
    else {
        match &args[1][..] {
            "-r" => {// Run
                assert!(args.len() > 2, "No simulation name argument");
                let mut sim = resources::load_sim(&args[2]);
                println!("Starting simulation");
                sim.main_loop();
            },
            "-n" => {// New
                println!("Creating new simulation\n");
                let name = extras::prompt("Name").trim().to_string();
                let size = extras::prompt("Side length (int)").trim().parse::<u32>().unwrap();//.expect("Invalid integer");
                let num_entities = extras::prompt("Number of each type of entity").trim().parse::<u32>().expect("Invalid integer");
                // entity type counts
                let mut entity_counts = HashMap::new();
                for type_ in EntityType::string_vector() {
                    entity_counts.insert(EntityType::from_string(&type_), num_entities);
                }
                // Create
                let mut sim = Simulation::new([size, size], entity_counts, name.clone());
                // Save
                resources::save_sim(&mut sim);
                resources::copy_default_settings(&name);
                println!("Created new simulation {name}");
            }
            _ => println!("Command(s) not recognized")
        }
    }
}
