use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    ops::{FnMut, DerefMut},
    result::Result,
    time::Instant,
    hash::Hash,
};

pub trait Categorized<C: Hash + Eq + PartialEq + Copy + Clone> {
    fn category(&self) -> Option<C>;
}
pub trait ValueStore<C: Hash + Eq + PartialEq + Copy + Clone, V: Categorized<C>> {
    fn get(&self, c: &C) -> V;
}

pub trait CBFn<G: ValueStore<C, V>, V: Categorized<C>, C: Hash + Eq + PartialEq + Copy + Clone>: FnMut(&G, &V, &Instant, &Instant) + Send + 'static {}
impl <T: FnMut(&G, &V, &Instant, &Instant) + Send + 'static, G: ValueStore<C, V>, V: Categorized<C>, C: Hash + Eq + PartialEq + Copy + Clone> CBFn<G, V, C> for T {}
pub struct CB<G, V: Categorized<C>, C: Hash + Eq + PartialEq + Copy + Clone> {
    c: C,
    cb: Weak<Mutex<CBFn<G, V, C>>>,
}
impl <G, V: Categorized<C>, C: Hash + Eq + PartialEq + Copy + Clone> CB<G, V, C> {
    fn call(&self, s: &G, v: &V, l_t: &Instant, c_t: &Instant) -> Result<(), ()> {
        match self.cb.upgrade() {
            Some(cb_m) => match cb_m.lock() {
                Ok(mut cb_mg) => Result::Ok((cb_mg.deref_mut())(s, v, l_t, c_t)),
                Err(_) => Result::Ok(()),
            },
            None => Result::Err(()),
        }
    }
    pub fn new(c: C, cb: Weak<Mutex<CBFn<G, V, C>>>) -> Self { CB { c: c, cb: cb } }
}
pub enum RegRequest<G, V: Categorized<C>, C: Hash + PartialEq + Eq + Copy + Clone> {
    Register(C, Vec<CB<G, V, C>>),
    Unregister(Vec<Weak<Mutex<CB<G, V, C>>>>),
}
pub enum RegResponse<G, V: Categorized<C>, C: Hash + PartialEq + Eq + Copy + Clone> {
    Register(C, Vec<Weak<Mutex<CB<G, V, C>>>>),
    Unregister,
}

pub struct Manager<G: ValueStore<C, V>, V: Categorized<C>, C: Hash + Eq + PartialEq + Copy + Clone> {
    // TODO  see if there's a better way.
    occupied: Vec<C>,
    // TODO potentially change Vec into a linked list for O(1) removal
    buckets: HashMap<C, Vec<Arc<Mutex<CB<G, V, C>>>>>,
    last_inst: Instant,
}
impl <G: ValueStore<C, V>, V: Categorized<C>, C: Hash + Eq + PartialEq + Copy + Clone> Manager<G, V, C> {
    pub fn new() -> Self {
        Manager {
            occupied: vec![],
            buckets: HashMap::new(),
            last_inst: Instant::now(),
        }
    }
    fn fire_event(s: &G, v: &V, cb_m: &Arc<Mutex<CB<G, V, C>>>, last_inst: &Instant, curr_inst: &Instant) -> Result<(), ()> {
        match cb_m.lock() {
            Ok(cb_mg) => (*cb_mg).call(s, v, last_inst, curr_inst),
            Err(_) => Result::Ok(()),
        }
    }
    fn fire_category_events(&self, s: &G, c: &C, v: &V, curr_inst: &Instant) -> (C, Vec<Arc<Mutex<CB<G, V, C>>>>) {
        let mut to_remove = vec![];
        if let Some(bucket) = self.buckets.get(&c) {
            for cb in bucket.iter() {
                match Manager::fire_event(s, v, cb, &self.last_inst, curr_inst) {
                    Ok(_) => (),
                    Err(_) => to_remove.push(cb.clone()),
                }
            }
        }
        (c.clone(), to_remove)
    }
    fn fire_all_events(&mut self, s: &G, curr_inst: &Instant) -> Vec<(C, Vec<Arc<Mutex<CB<G, V, C>>>>)> {
        let mut removal_stuff = Vec::with_capacity(self.occupied.len());
        for category in self.occupied.iter() {
            removal_stuff.push(self.fire_category_events(s, category, &s.get(&category), curr_inst));
        };
        removal_stuff
    }
    fn remove_category(cc: &mut Vec<C>, c: &C) {
        let idx = cc.iter().position(|x| x == c)
            .expect("Bug in data structure maintenance. ");
        cc.swap_remove(idx);
    }
    fn remove_matching_arcs<T>(v: &mut Vec<Arc<T>>, removing: Vec<Arc<T>>) {
        v.retain(|x| !removing.iter().any(|remove| Arc::ptr_eq(remove, x)))
    }
    fn remove_matching(&mut self, removals: Vec<(C, Vec<Arc<Mutex<CB<G, V, C>>>>)>) {
        for (c, removal) in removals {
            let bucket = self.buckets.get_mut(&c)
                .expect("Bug in data structure maintenance. Requested removal of nonexistent callbacks.");
            Self::remove_matching_arcs(bucket, removal);
            if bucket.len() == 0 { Self::remove_category(&mut self.occupied, &c); }
        }
    }
    pub fn register(&mut self, cb: CB<G, V, C>) -> Weak<Mutex<CB<G, V, C>>> {
        if !self.buckets.contains_key(&cb.c) { self.occupied.push(cb.c.clone()) }
        let v = self.buckets.entry(cb.c.clone()).or_insert(Vec::with_capacity(1));
        v.push(Arc::new(Mutex::new(cb)));
        Arc::downgrade(&v.last().unwrap())
    }
    fn unregister(&mut self, cb_m: Arc<Mutex<CB<G, V, C>>>) {
        let c = match cb_m.lock() {
            Ok(cb_mg) => cb_mg.c,
            Err(poisoned) => poisoned.into_inner().c,
        }.clone();
        self.remove_matching(vec![(c, vec![cb_m.clone()])]);
    }
    pub fn fire_and_clean_listing(&mut self, s: &G, vv: &mut Vec<V>) {
        let curr_inst = Instant::now();
        let mut deallocs = Vec::new();
        let mut dealloc_idx = HashMap::new();
        for v in vv.iter() {
            if let Some(c) = v.category() {
                let (c, mut rr) = self.fire_category_events(s, &c, &v, &curr_inst);
                if !rr.is_empty() {
                    let idx = dealloc_idx.entry(c.clone()).or_insert_with(|| {
                        deallocs.push((c, Vec::with_capacity(1)));
                        deallocs.len() - 1
                    }).clone();
                    deallocs[idx].1.append(&mut rr);
                }
            }
        }
        self.remove_matching(deallocs);
        self.last_inst = curr_inst;
    }
    pub fn fire_and_clean_all(&mut self, s: &G) {
        let curr_inst = Instant::now();
        let rems = self.fire_all_events(s, &curr_inst);
        self.remove_matching(rems);
        self.last_inst = curr_inst;
    }
    pub fn handle_req(&mut self, req: RegRequest<G, V, C>) -> RegResponse<G, V, C> {
        match req {
            RegRequest::Register(c, cbs) => {
                RegResponse::Register(c, cbs.into_iter().map(|cb| self.register(cb)).collect())
            },
            RegRequest::Unregister(cbs) => {
                for cb in cbs {
                    match cb.upgrade() {
                        Some(cb) => self.unregister(cb),
                        None => ()
                    }
                }
                RegResponse::Unregister
            },
        }
    }
}

