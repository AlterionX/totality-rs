use std::{cmp::Ordering, ops::{Index, IndexMut, Range}};

use phf::Mphf;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
struct STId(u64);
impl From<u64> for STId {
    fn from(a: u64) -> Self {
        Self(a)
    }
}
impl From<usize> for STId {
    fn from(a: usize) -> Self {
        Self(a as u64)
    }
}
impl From<STId> for u64 {
    fn from(a: STId) -> Self {
        a.0
    }
}
impl From<STId> for usize {
    fn from(a: STId) -> Self {
        a.0 as usize
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
struct OId(u64);
impl From<u64> for OId {
    fn from(a: u64) -> Self {
        Self(a)
    }
}
impl From<OId> for u64 {
    fn from(a: OId) -> Self {
        a.0
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub enum SortedTriEle {
    First, Second, Third
}
impl From<usize> for SortedTriEle {
    fn from(a: usize) -> Self {
        match a {
            0 => SortedTriEle::First,
            1 => SortedTriEle::Second,
            2 => SortedTriEle::Third,
            _ => panic!("Requested invalid member of SortedTriEle ({:?}).", a),
        }
    }
}
impl From<i32> for SortedTriEle {
    fn from(a: i32) -> Self {
        match a {
            0 => SortedTriEle::First,
            1 => SortedTriEle::Second,
            2 => SortedTriEle::Third,
            _ => panic!("Requested invalid member of SortedTriEle ({:?}).", a),
        }
    }
}
impl From<u64> for SortedTriEle {
    fn from(a: u64) -> Self {
        match a {
            0 => SortedTriEle::First,
            1 => SortedTriEle::Second,
            2 => SortedTriEle::Third,
            _ => panic!("Requested invalid member of SortedTriEle ({:?}).", a),
        }
    }
}
impl From<SortedTriEle> for u64 {
    fn from(a: SortedTriEle) -> Self {
        match a {
            SortedTriEle::First => 0,
            SortedTriEle::Second => 1,
            SortedTriEle::Third => 2,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct SortedTri {
    ordered: (STId, STId, STId),
}
impl<I: Into<SortedTriEle>> Index<I> for SortedTri {
    type Output = STId;
    fn index(&self, ord: I) -> &STId {
        match ord.into() {
            First => &self.ordered.0,
            Second => &self.ordered.1,
            Third => &self.ordered.2,
        }
    }
}
impl<I: Into<SortedTriEle>> IndexMut<I> for SortedTri {
    fn index_mut(&mut self, ord: I) -> &mut STId {
        match ord.into() {
            First => &mut self.ordered.0,
            Second => &mut self.ordered.1,
            Third => &mut self.ordered.2,
        }
    }
}

struct STAIBBin {
    tier: SortedTriEle,
    id_range: Range<usize>,
    val: u64,
}
impl STAIBBin {
    fn len(&self) -> usize {
        self.id_range.len()
    }
}
enum STAIBBucket<V> {
    Leaf(V),
    Hashed(Mphf<usize>, Vec<STAIBBucket<V>>),
}
impl<V> STAIBBucket<V> {
    fn lookup<W>(&self, k: usize) -> &STAIBBucket<V> {
        match self {
            STAIBBucket::Leaf(v) => self,
            STAIBBucket::Hashed(phf, buckets) => &buckets[phf.hash(&k) as usize],
        }
    }
}
pub struct SortedTriAndIntegerBimap {
    root_st: STAIBBucket<OId>,
    root_id: STAIBBucket<SortedTri>,
}
impl SortedTriAndIntegerBimap {
    fn find_bins(tier: usize, scan: &[(&SortedTri, OId)]) -> Vec<STAIBBin> {
        let tier = tier.into();
        let mut uniques = Vec::with_capacity(scan.len());
        let mut range_start = Vec::with_capacity(scan.len());
        let mut range_end = Vec::with_capacity(scan.len());
        let mut seen = 0;
        let mut last = 0;
        for (i, st) in scan.iter().enumerate() {
            if i == 0 || last != st.0[i].into() {
                seen += 1;
                last = st.0[i].into();
                uniques.push(last);
                range_start.push(i);
                if i != 1 {
                    range_end.push(i);
                }
            }
        }
        if scan.len() != 0 {
            range_end.push(scan.len());
        }
        (0..uniques.len()).map(|i| STAIBBin { tier: tier, id_range: range_start[i]..range_end[i], val: uniques[i] }).collect()
    }
    pub fn new(sts: &[SortedTri]) -> Self {
        const gamma: f64 = 1.0; // TODO tweak based on data set
        let sorted_sts = sts.iter().enumerate()
            .map(|(i, b)| (b, OId(i as u64)))
            .collect::<Vec<_>>();
        sorted_sts.sort_unstable_by(|a, b| a.cmp(b));
        let t0_buckets = Self::find_bins(0, sorted_sts.as_slice());
        // find minimal, costs O(n), oddly enough
        for t0_bucket in t0_buckets.iter() {
            let t1_buckets = Self::find_bins(1, &sorted_sts[t0_bucket.id_range]);
            if t0_bucket.len() == 1 {
            } else {
                for t1_bucket in t1_buckets.iter() {
                    if t1_bucket.len() == 1 {
                    } else {
                        let t2_buckets = Self::find_bins(2, &sorted_sts[t1_bucket.id_range]);
                        for t2_bucket in t2_buckets.iter() {
                            if t2_bucket.len() == 1 {
                                STAIBBucket::Leaf(t2_bucket.val);
                            } else {
                                let tup_ids = sorted_sts[t2_bucket.id_range].iter().map(|(a, _)| a[2]).collect();
                                let phf = Mphf::new(gamma, &tup_ids, None);
                            }
                            Mphf::new();
                        }
                    }
                }
                Mphf::new();
            }
        }
        Self { }
    }
    pub fn lookup_st(&self, st: &SortedTri) -> OId {
        let target = &self.root_st;
        for i in 0..3 {
            let k = st[i];
            let bucket = target.lookup(k.into());
            if let STAIBBucket::Leaf(v) = target {
                return *v;
            }
            target = bucket;
        }
        panic!("You can't get here! Or you're not supposed to anyways... (Input: {:?})", st);
    }
}
