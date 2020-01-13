use super::{base_table::BASE_TABLE, find_union, Kmer, DBGHMM, SCALE, THR, THR_ON};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
// use std::collections::HashMap;
#[derive(Default)]
pub struct Factory {
    inner: Vec<(f64, usize)>,
    is_safe: Vec<u8>,
    temp_index: Vec<usize>,
    edges: Vec<Vec<usize>>,
    fu: find_union::FindUnion,
    dfs_stack: Vec<usize>,
    dfs_flag: Vec<u8>,
    buffer: Vec<Kmer>,
}

fn to_u64(xs: &[u8]) -> usize {
    let mut sum = 0;
    for &x in xs {
        sum = sum << 2 | BASE_TABLE[x as usize];
    }
    sum
}

const TABLE: [u8; 4] = [b'A', b'C', b'G', b'T'];
#[allow(dead_code)]
fn decode(kmer: u64, k: usize) -> Vec<u8> {
    (0..k)
        .rev()
        .map(|digit| (kmer >> 2 * digit) & 0b11)
        .map(|base| TABLE[base as usize])
        .collect::<Vec<u8>>()
}

fn decode_to(kmer: u64, k: usize, buf: &mut Vec<u8>) {
    buf.clear();
    buf.extend(
        (0..k)
            .rev()
            .map(|digit| (kmer >> 2 * digit) & 0b11)
            .map(|base| TABLE[base as usize]),
    );
}

impl Factory {
    fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn clear(&mut self) {
        self.inner.clear();
        self.temp_index.clear();
        self.is_safe.clear();
        self.edges.iter_mut().for_each(|buf| buf.clear());
        self.fu.clear();
        self.dfs_stack.clear();
        self.dfs_flag.clear();
        self.buffer.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
            && self.is_safe.is_empty()
            && self.temp_index.is_empty()
            && self.edges.iter().all(|e| e.is_empty())
            && self.fu.is_empty()
            && self.dfs_stack.is_empty()
            && self.dfs_flag.is_empty()
            && self.buffer.is_empty()
    }
    pub fn generate(&mut self, dataset: &[Vec<u8>], k: usize) -> DBGHMM {
        assert!(k <= 32, "k should be less than 32.");
        let tk = 4u32.pow(k as u32) as usize;
        self.inner.clear();
        self.inner
            .extend(std::iter::repeat((0., std::usize::MAX)).take(tk));
        dataset.iter().for_each(|seq| {
            seq.windows(k).for_each(|kmer| {
                self.inner[to_u64(kmer)].0 += 1.;
            })
        });
        let thr = self.calc_thr();
        let mut nodes = Vec::with_capacity(self.len());
        for seq in dataset.iter().filter(|seq| seq.len() > k) {
            for x in seq.windows(k + 1) {
                if let Some((from, to)) = self.get_indices(x, &mut nodes, thr, k) {
                    nodes[from].push_edge_with(x[k], to);
                }
            }
            let x = self.inner[to_u64(&seq[seq.len() - k..])];
            if x.0 > thr && x.1 != std::usize::MAX {
                nodes[x.1].is_tail = true;
            }
            let x = self.inner[to_u64(&seq[..k])];
            if x.0 > thr && x.1 != std::usize::MAX {
                nodes[x.1].is_head = true;
            }
        }
        let nodes = Self::sort_nodes(nodes);
        let nodes = self.clean_up_nodes(nodes);
        let weight = dataset.len() as f64;
        DBGHMM::from(nodes, k, weight)
    }
    pub fn generate_from_ref(&mut self, dataset: &[&[u8]], k: usize) -> DBGHMM {
        assert!(k <= 32, "k should be less than 32.");
        // eprintln!("======DEPRECATED FUNCTION=======\nDO NOT CALL `generate_from_ref.`");
        let tk = 4u32.pow(k as u32) as usize;
        self.inner.clear();
        self.inner
            .extend(std::iter::repeat((0., std::usize::MAX)).take(tk));
        dataset.iter().for_each(|seq| {
            seq.windows(k).for_each(|kmer| {
                self.inner[to_u64(kmer)].0 += 1.;
            })
        });
        let thr = 0.;
        let mut nodes = Vec::with_capacity(self.len());
        for seq in dataset {
            for x in seq.windows(k + 1) {
                if let Some((from, to)) = self.get_indices(x, &mut nodes, thr, k) {
                    nodes[from].push_edge_with(x[k], to);
                }
            }
            let x = self.inner[to_u64(&seq[seq.len() - k..])];
            if x.0 > thr && x.1 != std::usize::MAX {
                nodes[x.1].is_tail = true;
            }
            let x = self.inner[to_u64(&seq[..k])];
            if x.0 > thr && x.1 != std::usize::MAX {
                nodes[x.1].is_head = true;
            }
        }
        let nodes = Self::sort_nodes(nodes);
        let weight = dataset.len() as f64;
        self.clear();
        DBGHMM::from(nodes, k, weight)
    }
    pub fn generate_with_weight(&mut self, dataset: &[&[u8]], ws: &[f64], k: usize) -> DBGHMM {
        assert!(k <= 32, "k should be less than 32.");
        assert!(self.is_empty());
        let tk = 4u32.pow(k as u32) as usize;
        let coverage = ws.iter().sum::<f64>();
        self.inner
            .extend(std::iter::repeat((0., std::usize::MAX)).take(tk));
        let mut rng: Xoshiro256StarStar = SeedableRng::seed_from_u64(dataset.len() as u64);
        assert_eq!(dataset.len(), ws.len());
        let ep = 0.0001;
        let mask = (1 << 2 * k) - 1;
        dataset
            .iter()
            .zip(ws.iter())
            .filter(|&(seq, &w)| w > ep && seq.len() >= k)
            .for_each(|(seq, w)| {
                let mut key = to_u64(&seq[..k - 1]);
                for &b in &seq[k - 1..] {
                    key = (key << 2 | BASE_TABLE[b as usize]) & mask;
                    self.inner[key].0 += w;
                }
            });
        let thr = self.calc_thr_weight();
        let mut nodes = Vec::with_capacity(1_000);
        let scale = SCALE * coverage.ln();
        for (seq, &w) in dataset.iter().zip(ws.iter()).filter(|&(_, &w)| w > ep) {
            let mut from = to_u64(&seq[..k]);
            for x in seq.windows(k + 1) {
                let b = x[k];
                let to = (from << 2 | BASE_TABLE[b as usize]) & mask;
                if let Some((from, to)) =
                    self.get_indices_exp(x, &mut nodes, thr, k, &mut rng, from, to, scale)
                {
                    nodes[from].push_edge_with_weight(x[k], to, w);
                }
                from = to;
            }
            let x = self.inner[to_u64(&seq[seq.len() - k..])];
            if x.1 != std::usize::MAX {
                nodes[x.1].is_tail = true;
            }
            let x = self.inner[to_u64(&seq[..k])];
            if x.1 != std::usize::MAX {
                nodes[x.1].is_head = true;
            }
        }
        let weight = ws.iter().sum::<f64>();
        if weight < 1.000 {
            self.clear();
            let nodes = vec![];
            return DBGHMM::from(nodes, k, weight);
        }
        let nodes = Self::sort_nodes(nodes);
        self.clear();
        let mut nodes = self.clean_up_nodes_exp(nodes);
        nodes.iter_mut().for_each(Kmer::finalize);
        self.clear();
        DBGHMM::from(nodes, k, weight)
    }
    pub fn generate_with_weight_prior(
        &mut self,
        dataset: &[&[u8]],
        ws: &[f64],
        k: usize,
        buf: &mut Vec<f64>,
    ) -> DBGHMM {
        buf.clear();
        let coverage = ws.iter().sum::<f64>();
        if coverage < 2. {
            return self.generate_with_weight(dataset, ws, k);
        }
        assert!(k <= 32, "k should be less than 32.");
        use super::PRIOR_FACTOR;
        let tk = (1 << (k * 2)) as usize;
        // let tk = 4u32.pow(k as u32) as usize;
        let weight = PRIOR_FACTOR * coverage;
        self.inner
            .extend(std::iter::repeat((weight, std::usize::MAX)).take(tk));
        buf.extend(std::iter::repeat(weight).take(tk * 4));
        let seed = ws.iter().sum::<f64>().floor().abs() as u64 + dataset.len() as u64;
        let mut rng: Xoshiro256StarStar = SeedableRng::seed_from_u64(seed);
        let ep = 0.000000001;
        let mask = (1 << 2 * k) - 1;
        let e_mask = (1 << 2 * (k + 1)) - 1;
        // Record all the k-mers and k+1-mers.
        dataset
            .iter()
            .zip(ws.iter())
            .filter(|&(seq, _)| seq.len() > k)
            .for_each(|(seq, w)| {
                let mut node = to_u64(&seq[..k]);
                self.inner[node].0 += w;
                let mut edge = to_u64(&seq[..k]);
                for &b in &seq[k..] {
                    node = (node << 2 | BASE_TABLE[b as usize]) & mask;
                    self.inner[node].0 += w;
                    edge = (edge << 2 | BASE_TABLE[b as usize]) & e_mask;
                    buf[edge] += w;
                }
                self.inner[node].0 += w;
            });
        let thr = {
            let sum = self.inner.iter().map(|e| e.0).sum::<f64>();
            THR * sum / self.inner.len() as f64
        };
        let mut nodes = Vec::with_capacity(2_500);
        let mut edge = vec![];
        let scale = SCALE * coverage.ln();
        for (e, &w) in buf.iter().enumerate().filter(|&(_, &w)| w > ep) {
            let (from, to) = (e >> 2, e & mask);
            decode_to(e as u64, k + 1, &mut edge);
            if let Some((from, to)) =
                self.get_indices_exp(&edge, &mut nodes, thr, k, &mut rng, from, to, scale)
            {
                nodes[from].push_edge_with_weight(edge[k], to, w);
            }
        }
        for (seq, _) in dataset
            .iter()
            .zip(ws.iter())
            .filter(|&(seq, &w)| w > ep && seq.len() > k)
        {
            let x = self.inner[to_u64(&seq[seq.len() - k..])];
            if x.1 != std::usize::MAX {
                nodes[x.1].is_tail = true;
            }
            let x = self.inner[to_u64(&seq[..k])];
            if x.1 != std::usize::MAX {
                nodes[x.1].is_head = true;
            }
        }
        let nodes = Self::sort_nodes(nodes);
        // Here, we record the base counts.
        self.clear();
        let mut nodes = self.clean_up_nodes_exp(nodes);
        nodes.iter_mut().for_each(Kmer::finalize);
        self.clear();
        DBGHMM::from(nodes, k, coverage)
    }
    #[allow(dead_code)]
    fn finalize(&mut self, mut nodes: Vec<Kmer>, k: usize) -> Vec<Kmer> { // 
        self.clear();
        // Construct the graph.
        for (from, node) in nodes.iter().enumerate() {
            for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
                self.edges[from].push(to);
            }
        }
        let len = nodes.len();
        nodes.iter_mut().enumerate().for_each(|(idx, node)| {
            if self.edges[idx].len() <= 1 {
                node.finalize();
            } else {
                // DFS
                let which = self.select_margiable_edges(&node, k - 1, len);
                node.finalize_global(&which);
            }
        });
        nodes
    }
    fn select_margiable_edges(&mut self, node: &Kmer, k: usize, len: usize) -> [bool; 4] {
        assert!(self.is_safe.is_empty() && self.dfs_flag.is_empty() && self.dfs_stack.is_empty());
        (0..len).for_each(|_| self.is_safe.push(0));
        (0..len).for_each(|_| self.dfs_flag.push(0));
        for (idx, to) in node.edges.iter().enumerate() {
            let to = match to {
                Some(to) => to,
                None => continue,
            };
            // Depth-limited dfs from to, checking the nodes arrived.
            self.depth_limited_dfs(k, *to, idx);
        }
        let mut which = [false; 4];
        for &flag in self.is_safe.iter() {
            if flag.count_ones() > 1 {
                if flag & 0b0001 != 0 {
                    which[0] = true;
                }
                if flag & 0b0010 != 0 {
                    which[1] = true;
                }
                if flag & 0b0100 != 0 {
                    which[2] = true;
                }
                if flag & 0b1000 != 0 {
                    which[3] = true;
                }
            }
        }
        self.is_safe.clear();
        self.dfs_flag.clear();
        self.dfs_stack.clear();
        which
    }
    fn depth_limited_dfs(&mut self, limit: usize, start: usize, idx: usize) {
        assert!(self.dfs_stack.is_empty());
        assert!(self.dfs_flag.iter().all(|&e| e == 0));
        let bit = match idx {
            0 => 0b0001,
            1 => 0b0010,
            2 => 0b0100,
            3 => 0b1000,
            _ => unreachable!(),
        };
        self.dfs_stack.push(start);
        let mut current_depth = 1;
        'dfs: while !self.dfs_stack.is_empty() {
            let s = *self.dfs_stack.last().unwrap();
            self.dfs_flag[s] = 1;
            self.is_safe[s] |= bit;
            for &to in self.edges[s].iter() {
                if self.dfs_flag[to] == 0 && current_depth <= limit {
                    self.dfs_stack.push(to);
                    current_depth += 1;
                    continue 'dfs;
                }
            }
            self.dfs_stack.pop().unwrap();
            current_depth -= 1;
        }
        self.dfs_flag.iter_mut().for_each(|e| *e = 0);
    }
    fn sort_nodes(mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        let mut positions: Vec<(usize, bool)> =
            nodes.iter().map(|e| e.is_head).enumerate().collect();
        let cmp = |&(_, a): &(usize, bool), &(_, b): &(usize, bool)| match (a, b) {
            (true, true) | (false, false) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
        };
        positions.sort_by(cmp);
        let next_position: Vec<usize> = {
            let mut pos = vec![0; nodes.len()];
            for (next_pos, (previous, _)) in positions.into_iter().enumerate() {
                pos[previous] = next_pos;
            }
            pos
        };
        let mut result = vec![None; nodes.len()];
        let mut idx = nodes.len();
        while let Some(mut node) = nodes.pop() {
            idx -= 1;
            node.rename_by(&next_position);
            result[next_position[idx]] = Some(node);
        }
        assert!(result.iter().all(|e| e.is_some()));
        assert!(nodes.is_empty());
        nodes.extend(result.into_iter().filter_map(|e| e));
        nodes
    }

    fn clean_up_nodes(&mut self, nodes: Vec<Kmer>) -> Vec<Kmer> {
        let mut nodes = self.renaming_nodes(nodes);
        nodes.iter_mut().for_each(Kmer::finalize);
        nodes
    }
    fn clean_up_nodes_exp(&mut self, nodes: Vec<Kmer>) -> Vec<Kmer> {
        assert!(self.is_empty());
        let nodes = self.trim_unreachable_nodes(nodes);
        let nodes = self.trim_unreachable_nodes_reverse(nodes);
        let save = nodes.clone();
        let nodes = self.bridge_pruning(nodes);
        let nodes = self.bridge_pruning(nodes);
        let nodes = self.trim_unreachable_nodes(nodes);
        let nodes = self.trim_unreachable_nodes_reverse(nodes);
        let save = if nodes.len() < 130 {
            // debug!(
            //     "Model Broken by bridge pruning:{}->{}",
            //     save.len(),
            //     nodes.len()
            // );
            let nodes = self.pick_largest_components(save);
            let nodes = self.renaming_nodes(nodes);
            return nodes;
        } else {
            nodes.clone()
        };
        let nodes = self.select_head_and_tail(nodes);
        let nodes = self.filter_lightweight(nodes);
        let nodes = self.trim_unreachable_nodes(nodes);
        let nodes = self.trim_unreachable_nodes_reverse(nodes);
        if nodes.len() < 130 {
            // debug!("Model Broken at select h/t:{}->{}", save.len(), nodes.len());
            // let b_head = save.iter().filter(|e| e.is_head).count();
            // let b_tail = save.iter().filter(|e| e.is_tail).count();
            // let head = nodes.iter().filter(|e| e.is_head).count();
            // let tail = nodes.iter().filter(|e| e.is_tail).count();
            // debug!("{}:{}->{}:{}", b_head, b_tail, head, tail);
            let nodes = self.pick_largest_components(save);
            let nodes = self.renaming_nodes(nodes);
            nodes
        } else {
            //debug!("OK!:{}->{}", save.len(), nodes.len());
            let nodes = self.pick_largest_components(nodes);
            let nodes = self.renaming_nodes(nodes);
            nodes
        }
    }
    // fn trim_ht_edges(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
    //     let mut buf = Vec::with_capacity(4);
    //     self.clear();
    //     // Remove non-head -> head edges.
    //     nodes
    //         .iter()
    //         .for_each(|n| self.is_safe.push(n.is_head as u8));
    //     for node in nodes.iter_mut().filter(|e| !e.is_head) {
    //         let removes = node
    //             .edges
    //             .iter()
    //             .enumerate()
    //             .filter_map(|(idx, to)| to.map(|to| (idx, to)))
    //             .filter(|(_, to)| self.is_safe[*to] == 1)
    //             .map(|(idx, _)| idx);
    //         buf.extend(removes);
    //         while let Some(idx) = buf.pop() {
    //             node.remove(idx);
    //         }
    //     }
    //     self.is_safe.clear();
    //     nodes
    //         .iter()
    //         .for_each(|n| self.is_safe.push(n.is_tail as u8));
    //     for node in nodes.iter_mut().filter(|e| e.is_tail) {
    //         let removes = node
    //             .edges
    //             .iter()
    //             .enumerate()
    //             .filter_map(|(idx, to)| to.map(|to| (idx, to)))
    //             .filter(|(_, to)| self.is_safe[*to] == 0)
    //             .map(|(idx, _)| idx);
    //         buf.extend(removes);
    //         while let Some(idx) = buf.pop() {
    //             node.remove(idx);
    //         }
    //     }
    //     self.clear();
    //     nodes
    // }
    fn bridge_pruning(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        let mut buffer: Vec<(usize, usize)> = Vec::with_capacity(4);
        let (lowlink, orders) = self.lowlink_and_orders(&nodes);
        let sum = nodes
            .iter()
            .map(|e| e.weight.iter().sum::<f64>())
            .sum::<f64>();
        let edges = nodes
            .iter()
            .map(|e| e.edges.iter().filter(|e| e.is_some()).count())
            .sum::<usize>();
        let ave = sum / edges as f64;
        for (from, node) in nodes.iter_mut().enumerate() {
            buffer.clear();
            let no_bridges = node
                .edges
                .iter()
                .enumerate()
                .filter_map(|(idx, e)| e.map(|to| (idx, to)))
                .filter(|&(_, to)| {
                    let is_bridge = orders[from] < lowlink[to] || orders[to] < lowlink[from];
                    !is_bridge
                });
            buffer.extend(no_bridges);
            let number_of_non_bridges = buffer.len();
            if number_of_non_bridges <= 1 {
                continue;
            }
            // Let's remove only one edge: the lightest,
            // if the edge is very light.
            let factor = (1. + (-ave / 30.).exp()).recip() - 0.5;
            let thr = 0.4 + 0.2 * factor;
            let (idx, w) = buffer.iter().fold((0, 10_000.), |(idx, min), &(i, _)| {
                let w = node.weight[i];
                if w < min {
                    (i, w)
                } else {
                    (idx, min)
                }
            });
            if w < ave * thr {
                node.remove(idx);
            }
        }
        nodes
    }
    fn lowlink_and_orders(&mut self, nodes: &[Kmer]) -> (Vec<usize>, Vec<usize>) {
        self.clear();
        self.dfs_flag.extend(std::iter::repeat(0).take(nodes.len()));
        if self.edges.len() < nodes.len() {
            (0..nodes.len() - self.edges.len()).for_each(|_| self.edges.push(vec![]));
        }
        for (from, node) in nodes.iter().enumerate() {
            for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
                self.edges[from].push(to);
                self.edges[to].push(from);
            }
        }
        let nodes = nodes.len();
        self.dfs_flag.extend(std::iter::repeat(0).take(nodes));
        let mut lowlink: Vec<usize> = vec![1000000000; nodes];
        let mut orders: Vec<usize> = vec![0; nodes];
        let mut order = 0;
        let mut is_dfs_edge: Vec<_> = self.edges.iter().map(|es| vec![false; es.len()]).collect();
        for i in 0..nodes {
            if self.dfs_flag[i] != 0 {
                continue;
            }
            self.dfs_stack.push(i);
            // Root node
            'outer: while !self.dfs_stack.is_empty() {
                let node = *self.dfs_stack.last().unwrap();
                if self.dfs_flag[node] == 0 {
                    self.dfs_flag[node] = 1;
                    orders[node] = order;
                    lowlink[node] = order;
                    order += 1;
                }
                for (idx, &to) in self.edges[node].iter().enumerate() {
                    if self.dfs_flag[to] == 0 {
                        self.dfs_stack.push(to);
                        is_dfs_edge[node][idx] = true;
                        continue 'outer;
                    }
                }
                let last = self.dfs_stack.pop().unwrap();
                let parent = *self.dfs_stack.last().unwrap_or(&10000000);
                for (&to, &is_dfs) in self.edges[last].iter().zip(is_dfs_edge[node].iter()) {
                    if to == parent {
                        continue;
                    } else if is_dfs {
                        let low = lowlink[to];
                        lowlink[last] = lowlink[last].min(low);
                    } else {
                        lowlink[last] = lowlink[last].min(orders[to]);
                    }
                }
            }
        }
        self.clear();
        (lowlink, orders)
    }
    #[allow(dead_code)]
    fn relative_weight_pruning(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        let ave = nodes.iter().map(|e| e.tot).sum::<f64>() / nodes.len() as f64;
        let thr = 0.5 * ave;
        let is_weak: Vec<_> = nodes
            .iter()
            .map(|e| e.tot < thr && !e.is_head && !e.is_tail)
            .collect();
        // Maybe we should do coverage tuning.
        let mut buf = vec![];
        for (from, node) in nodes.iter_mut().enumerate() {
            node.edges
                .iter()
                .enumerate()
                .filter_map(|(idx, e)| e.map(|to| (idx, to)))
                .for_each(|(idx, to)| buf.push((idx, to)));
            while let Some((idx, to)) = buf.pop() {
                if is_weak[from] && is_weak[to] {
                    node.remove(idx);
                }
            }
        }
        nodes
    }
    fn filter_lightweight(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        self.clear();
        let len = nodes.len() as f64;
        let ws = nodes.iter().map(|e| e.kmer_weight);
        let mean = ws.clone().sum::<f64>() / len;
        // let factor = THR - (1. + (mean / 10.).exp()).recip() / 2.;
        let thr = mean / THR;
        nodes
            .iter()
            .for_each(|e| self.is_safe.push((e.kmer_weight > thr) as u8));
        let mut index = 0;
        for &b in &self.is_safe {
            self.temp_index.push(index);
            index += b as usize;
        }
        assert!(self.buffer.is_empty());
        let mut idx = nodes.len();
        while let Some(mut node) = nodes.pop() {
            idx -= 1;
            if self.is_safe[idx] == 1 {
                node.remove_if_not_supported(&self.is_safe);
                node.rename_by(&self.temp_index);
                self.buffer.push(node);
            }
        }
        self.buffer.reverse();
        std::mem::swap(&mut nodes, &mut self.buffer);
        self.clear();
        nodes
    }
    fn trim_unreachable_nodes(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        self.clear();
        self.dfs_flag.extend(std::iter::repeat(0).take(nodes.len()));
        nodes.iter().enumerate().for_each(|(idx, n)| {
            if n.is_head {
                self.dfs_stack.push(idx)
            }
        });
        if self.dfs_stack.is_empty() {
            // We should pick some "head" nodes.
            self.is_safe.clear();
            self.is_safe.extend(std::iter::repeat(1).take(nodes.len()));
            for n in nodes.iter() {
                for &to in n.edges.iter().filter_map(|e| e.as_ref()) {
                    self.is_safe[to] = 0;
                }
            }
            self.dfs_stack
                .extend(self.is_safe.iter().enumerate().filter_map(|(idx, &is_ok)| {
                    if is_ok == 1 {
                        Some(idx)
                    } else {
                        None
                    }
                }));
            self.is_safe.clear();
            if self.dfs_stack.is_empty() {
                self.clear();
                return nodes;
            }
        }
        'dfs: while !self.dfs_stack.is_empty() {
            let node = *self.dfs_stack.last().unwrap();
            self.dfs_flag[node] = 1;
            for &to in nodes[node].edges.iter().filter_map(|e| e.as_ref()) {
                if self.dfs_flag[to] == 0 {
                    self.dfs_stack.push(to);
                    continue 'dfs;
                }
            }
            self.dfs_stack.pop().unwrap();
        }
        let mut index = 0;
        for &b in &self.dfs_flag {
            self.temp_index.push(index);
            index += b as usize;
        }
        assert!(self.buffer.is_empty());
        let mut idx = nodes.len();
        while let Some(mut node) = nodes.pop() {
            idx -= 1;
            if self.dfs_flag[idx] == 1 {
                node.remove_if_not_supported(&self.dfs_flag);
                node.rename_by(&self.temp_index);
                self.buffer.push(node);
            }
        }
        self.buffer.reverse();
        std::mem::swap(&mut nodes, &mut self.buffer);
        self.clear();
        nodes
    }
    fn trim_unreachable_nodes_reverse(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        self.clear();
        self.dfs_flag.extend(std::iter::repeat(0).take(nodes.len()));
        self.dfs_stack
            .extend(nodes.iter().enumerate().filter_map(
                |(idx, n)| {
                    if n.is_tail {
                        Some(idx)
                    } else {
                        None
                    }
                },
            ));
        if self.dfs_stack.is_empty() {
            // We should pick some "tail" nodes.
            self.is_safe.clear();
            self.is_safe.extend(std::iter::repeat(0).take(nodes.len()));
            for (from, n) in nodes.iter().enumerate() {
                self.is_safe[from] = n.edges.iter().all(|e| e.is_none()) as u8;
            }
            self.dfs_stack
                .extend(self.is_safe.iter().enumerate().filter_map(|(idx, &is_ok)| {
                    if is_ok == 1 {
                        Some(idx)
                    } else {
                        None
                    }
                }));
            self.is_safe.clear();
            if self.dfs_stack.is_empty() {
                // debug!(
                //     "REVERSE:This graph is st-connected. Total Weight:{}",
                //     nodes.iter().map(|e| e.tot).sum::<f64>()
                // );
                self.clear();
                return nodes;
            }
        }
        let mut edges: Vec<Vec<usize>> = vec![Vec::with_capacity(4); nodes.len()];
        for (from, node) in nodes.iter().enumerate() {
            for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
                //Reverse edges!
                edges[to].push(from);
            }
        }
        'dfs: while !self.dfs_stack.is_empty() {
            let node = *self.dfs_stack.last().unwrap();
            self.dfs_flag[node] = 1;
            for &to in &edges[node] {
                if self.dfs_flag[to] == 0 {
                    self.dfs_stack.push(to);
                    continue 'dfs;
                }
            }
            self.dfs_stack.pop().unwrap();
        }
        let mut index = 0;
        for &b in &self.dfs_flag {
            self.temp_index.push(index);
            index += b as usize;
        }
        assert!(self.buffer.is_empty());
        let mut idx = nodes.len();
        while let Some(mut node) = nodes.pop() {
            idx -= 1;
            if self.dfs_flag[idx] == 1 {
                node.remove_if_not_supported(&self.dfs_flag);
                node.rename_by(&self.temp_index);
                self.buffer.push(node);
            }
        }
        self.buffer.reverse();
        std::mem::swap(&mut nodes, &mut self.buffer);
        self.clear();
        nodes
    }
    #[allow(dead_code)]
    fn indegree_removing(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        self.is_safe.clear();
        self.temp_index.clear();
        self.is_safe.extend(std::iter::repeat(0).take(nodes.len()));
        // Record indegree
        nodes.iter().for_each(|_| self.temp_index.push(0));
        for node in nodes.iter() {
            for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
                self.temp_index[to] += 1;
            }
        }
        let tot = nodes
            .iter()
            .map(|e| e.weight.iter().sum::<f64>())
            .sum::<f64>();
        let len = nodes
            .iter()
            .map(|e| e.edges.iter().filter(|e| e.is_some()).count())
            .sum::<usize>();
        let thr = tot / len as f64 / THR;
        let mut buf = vec![];
        for node in nodes.iter_mut() {
            let edges = node
                .edges
                .iter()
                .enumerate()
                .filter_map(|(idx, e)| e.map(|to| (idx, to)));
            let num_edges = edges.clone().count();
            if num_edges <= 1 {
                continue;
            }
            edges
                .filter(|&(idx, to)| self.temp_index[to] > 1 && node.weight[idx] < thr)
                .for_each(|e| buf.push(e));
            while let Some((idx, _)) = buf.pop() {
                node.remove(idx);
            }
        }
        nodes
    }
    #[allow(dead_code)]
    fn cut_lightweight_loop(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        self.clear();
        self.is_safe.extend(std::iter::repeat(0).take(nodes.len()));
        // Record indegree to is_safe.
        for node in nodes.iter() {
            for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
                self.is_safe[to] += 1;
            }
        }
        // Record outdegrees.
        let outdegrees: Vec<_> = nodes
            .iter()
            .map(|node| node.edges.iter().filter(|e| e.is_some()).count())
            .collect();
        // Calculate the length of the simple path it is recorded to temp_index.
        // Push putative start nodes.
        self.dfs_stack.clear();
        for (idx, node) in nodes.iter().enumerate() {
            let outdegree = outdegrees[idx];
            let indegree = self.is_safe[idx];
            if indegree == 0 && outdegree == 1 {
                // This is a start of the simple path.
                self.dfs_stack.push(idx);
            } else if indegree > 1 || outdegree > 1 {
                // The children of this node could be starts nodes of a simple path.
                for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
                    let indegree = self.is_safe[to];
                    let outdegree = outdegrees[to];
                    if indegree < 2 && outdegree < 2 {
                        self.dfs_stack.push(idx);
                    }
                }
            }
        }
        // Pop putative simple path one by one.
        // Note that a node never reached twice or more.
        self.temp_index
            .extend(std::iter::repeat(0).take(nodes.len()));
        while let Some(start) = self.dfs_stack.pop() {
            // succeed until reach a branching node.
            let mut current = start;
            let mut stack = vec![];
            while self.is_safe[current] < 2 && outdegrees[current] < 2 {
                stack.push(current);
                let next = nodes[current]
                    .edges
                    .iter()
                    .filter_map(|e| e.as_ref())
                    .nth(0);
                if let Some(&next) = next {
                    current = next;
                } else {
                    break;
                }
            }
            // Determine the length of the simple path, and set the length of the simple path.
            let len = stack.len();
            while let Some(node) = stack.pop() {
                assert!(self.temp_index[node] == 0);
                self.temp_index[node] = len;
            }
        }
        // We treat the tail node as special case.
        self.is_safe.clear();
        nodes
            .iter()
            .for_each(|e| self.is_safe.push(e.is_tail as u8));
        for node in nodes.iter_mut() {
            let max = *node
                .weight
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(&0.);
            let factor = THR; // * (1. + -max.exp()).recip();
            let thr = max / factor;
            for idx in 0..4 {
                let to = match node.edges[idx] {
                    Some(res) => res,
                    None => continue,
                };
                let is_not_tail = self.is_safe[to] != 1;
                if self.temp_index[to] <= 2 && node.weight[idx] < thr && is_not_tail {
                    // eprintln!("{:?}Remove{}({:.3})", node, idx, thr);
                    node.remove(idx);
                }
            }
        }
        self.clear();
        nodes
    }
    fn select_head_and_tail(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        let sum = nodes.iter().map(|e| e.kmer_weight).sum::<f64>();
        let thr = sum / nodes.len() as f64 / 2.5;
        self.is_safe.clear();
        self.is_safe.extend(std::iter::repeat(1).take(nodes.len()));
        self.temp_index.clear();
        for (idx, n) in nodes.iter().enumerate() {
            if (n.is_head || n.is_tail) && n.kmer_weight < thr {
                self.is_safe[idx] = 0;
            }
        }
        let mut index = 0;
        for &b in &self.is_safe {
            self.temp_index.push(index);
            index += b as usize;
        }
        assert!(self.buffer.is_empty());
        let mut idx = nodes.len();
        while let Some(mut node) = nodes.pop() {
            idx -= 1;
            if self.is_safe[idx] == 1 {
                node.remove_if_not_supported(&self.is_safe);
                node.rename_by(&self.temp_index);
                self.buffer.push(node);
            }
        }
        self.buffer.reverse();
        self.is_safe.clear();
        self.temp_index.clear();
        std::mem::swap(&mut nodes, &mut self.buffer);
        nodes
    }

    // // Cut tips. We can assume that the graph is a
    // // connected graph or a tree.
    // #[allow(dead_code)]
    // fn cut_tip(&mut self, mut nodes: Vec<Kmer>, t: usize, thr: f64) -> Vec<Kmer> {
    //     assert!(self.is_safe.is_empty());
    //     assert!(self.edges.iter().all(|e| e.is_empty()));
    //     let pseudo_node = nodes.len();
    //     if self.edges.len() < nodes.len() + 1 {
    //         let l = nodes.len() + 1 - self.edges.len();
    //         (0..=l).for_each(|_| self.edges.push(Vec::with_capacity(4)));
    //     }
    //     nodes.iter().for_each(|_| self.is_safe.push(0));
    //     for (idx, node) in nodes.iter().enumerate() {
    //         for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
    //             self.edges[idx].push(to);
    //         }
    //         if node.is_tail {
    //             self.edges[idx].push(pseudo_node);
    //         }
    //     }
    //     // By calling this function, self.is_safe[i] would be changed!
    //     self.cut_tip_inner(nodes.len() + 1, t, 0b010);
    //     self.edges.iter_mut().for_each(|eds| eds.clear());
    //     nodes
    //         .iter()
    //         .zip(self.is_safe.iter_mut())
    //         .for_each(|(n, b)| *b |= n.is_head as u8);
    //     for (idx, node) in nodes.iter().enumerate() {
    //         for &to in node.edges.iter().filter_map(|e| e.as_ref()) {
    //             self.edges[to].push(idx);
    //         }
    //         if node.is_head {
    //             self.edges[idx].push(pseudo_node);
    //         }
    //     }
    //     self.cut_tip_inner(nodes.len() + 1, t, 0b100);
    //     nodes
    //         .iter()
    //         .zip(self.is_safe.iter_mut())
    //         .for_each(|(n, b)| {
    //             *b |= (n.kmer_weight > thr) as u8;
    //         });
    //     self.is_safe.iter_mut().for_each(|is_safe| {
    //         let high_coverage = (*is_safe & 0b001) == 0b001;
    //         let not_cut_by_sweep = (*is_safe & 0b110) == 0b110;
    //         *is_safe = (high_coverage | not_cut_by_sweep) as u8;
    //     });
    //     assert!(self.temp_index.is_empty());
    //     let mut idx = 0;
    //     for &b in &self.is_safe {
    //         self.temp_index.push(idx);
    //         idx += b as usize;
    //     }
    //     assert!(self.buffer.is_empty());
    //     let mut idx = nodes.len();
    //     while let Some(mut node) = nodes.pop() {
    //         idx -= 1;
    //         if self.is_safe[idx] == 1 {
    //             node.remove_if_not_supported(&self.is_safe);
    //             node.rename_by(&self.temp_index);
    //             self.buffer.push(node)
    //         }
    //     }
    //     self.buffer.reverse();
    //     std::mem::swap(&mut self.buffer, &mut nodes);
    //     self.is_safe.clear();
    //     self.temp_index.clear();
    //     self.edges.iter_mut().for_each(|e| e.clear());
    //     self.buffer.clear();
    //     nodes
    // }
    // // Cut tips. We can assume that the graph is a
    // // connected graph or a tree.
    // // We use 'flag' instead of 'true'
    // #[allow(dead_code)]
    // fn cut_tip_inner(&mut self, nodes: usize, t: usize, flag: u8) {
    //     // Use self.temp_index as a distance
    //     // In other words, self.temp_index[i] = "Distance from root to i-th node"
    //     self.dist_to_root(nodes);
    //     // Allocate All the bad nodes into dfs stack. Then write them back to edges list.
    //     // We use the last packed of edge list to keep list of bad nodes.
    //     let bad_list: Vec<_> = (0..nodes)
    //         .filter(|&e| self.edges[e].len() > 1)
    //         .flat_map(|e| self.edges[e].iter().filter(|&&to| self.temp_index[to] <= t))
    //         .copied()
    //         .collect();
    //     self.temp_index.clear();
    //     assert!(self.dfs_flag.is_empty());
    //     // 0 <-> Not arrived
    //     // 1 <-> Have arrived
    //     self.dfs_flag.extend(std::iter::repeat(0).take(nodes));
    //     for i in bad_list {
    //         if self.dfs_flag[i] == 1 {
    //             continue;
    //         }
    //         self.dfs_stack.push(i);
    //         'dfs: while !self.dfs_stack.is_empty() {
    //             let node = *self.dfs_stack.last().unwrap();
    //             self.dfs_flag[node] = 1;
    //             for &to in &self.edges[node] {
    //                 if self.dfs_flag[to] == 0 {
    //                     self.dfs_stack.push(to);
    //                     continue 'dfs;
    //                 }
    //             }
    //             self.dfs_stack.pop().unwrap();
    //         }
    //     }
    //     // flag * (1-e) = flag if the i-th node was not traversed,
    //     // otherwise, it is zero. Thus, by BitOR, the flag would be
    //     // modificated as supposed to be.
    //     self.dfs_flag
    //         .iter()
    //         .zip(self.is_safe.iter_mut())
    //         .for_each(|(&e, is_safe)| *is_safe |= flag * (1 - e));
    //     self.dfs_flag.clear();
    //     assert!(self.dfs_stack.is_empty());
    // }
    // #[allow(dead_code)]
    // fn dist_to_root(&mut self, nodes: usize) {
    //     // 0 -> never arrived
    //     // 1 -> active(arrived, being traversed currently)
    //     // 2 -> inactive(arrived, have been traversed)
    //     self.dfs_flag.extend(std::iter::repeat(0).take(nodes));
    //     assert!(self.temp_index.is_empty());
    //     self.temp_index.extend(std::iter::repeat(0).take(nodes));
    //     for i in 0..nodes {
    //         if self.dfs_flag[i] != 0 {
    //             continue;
    //         }
    //         self.dfs_stack.push(i);
    //         'dfs: while !self.dfs_stack.is_empty() {
    //             let node = *self.dfs_stack.last().unwrap();
    //             if self.dfs_flag[node] == 0 {
    //                 self.dfs_flag[node] = 1;
    //             }
    //             for &to in &self.edges[node] {
    //                 if self.dfs_flag[to] == 0 {
    //                     self.dfs_stack.push(to);
    //                     continue 'dfs;
    //                 } else if self.dfs_flag[to] == 1 {
    //                     // Loop detected. Some big number.
    //                     self.temp_index[node] = 100_000;
    //                 }
    //             }
    //             let last = self.dfs_stack.pop().unwrap();
    //             // The last .max(dist_to_root[last]) is indeed needed,
    //             // as there would be a certain amount of cycles.
    //             self.temp_index[last] = self.edges[node]
    //                 .iter()
    //                 .map(|&to| self.temp_index[to] + 1)
    //                 .max()
    //                 .unwrap_or(0)
    //                 .max(self.temp_index[last]);
    //             // Deactivate
    //             self.dfs_flag[last] = 2;
    //         }
    //     }
    //     self.dfs_flag.clear();
    // }
    fn pick_largest_components(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        assert!(self.is_empty());
        self.fu.refresh(nodes.len());
        for (from, n) in nodes.iter().enumerate() {
            for &to in n.edges.iter().filter_map(|e| e.as_ref()) {
                self.fu.unite(from, to);
            }
        }
        let max_group_and_size = (0..nodes.len())
            .filter_map(|e| {
                let parent = self.fu.find(e).unwrap();
                if parent == e {
                    self.fu.size(e).map(|s| (e, s))
                } else {
                    None
                }
            })
            .max_by_key(|&(_, size)| size);
        let (max_group, _) = match max_group_and_size {
            Some(res) => res,
            None => panic!("No components:{:?}", nodes),
        };
        let mut index = 0;
        for i in 0..nodes.len() {
            self.temp_index.push(index);
            index += (self.fu.find(i).unwrap() == max_group) as usize;
        }
        let mut index = nodes.len();
        while let Some(mut node) = nodes.pop() {
            index -= 1;
            if self.fu.find(index).unwrap() == max_group {
                node.remove_if_not(&mut self.fu, max_group);
                node.rename_by(&self.temp_index);
                self.buffer.push(node);
            }
        }
        self.buffer.reverse();
        std::mem::swap(&mut self.buffer, &mut nodes);
        self.clear();
        nodes
    }
    fn renaming_nodes(&mut self, mut nodes: Vec<Kmer>) -> Vec<Kmer> {
        self.topological_sort(&nodes);
        let mut result = vec![None; nodes.len()];
        let mut idx = nodes.len();
        while let Some(mut node) = nodes.pop() {
            idx -= 1;
            node.rename_by(&self.temp_index);
            result[self.temp_index[idx]] = Some(node);
        }
        assert!(result.iter().all(|e| e.is_some()));
        assert!(nodes.is_empty());
        self.clear();
        nodes.extend(result.into_iter().filter_map(|e| e));
        nodes
    }
    // Return topological sorted order. If there's a cycle,
    // it neglect the back-edge, and proceed with raise error messages to stderr(DEBUG MODE).
    fn topological_sort(&mut self, nodes: &[Kmer]) {
        if self.edges.len() < nodes.len() {
            let l = nodes.len() - self.edges.len();
            self.edges
                .extend(std::iter::repeat(Vec::with_capacity(4)).take(l));
        }
        for (idx, node) in nodes.iter().enumerate() {
            for to in node.edges.iter().filter_map(|e| e.as_ref()) {
                self.edges[idx].push(*to);
            }
        }
        self.topological_sort_inner(nodes.len())
    }
    // Topological sorting.
    fn topological_sort_inner(&mut self, nodes: usize) {
        // 0 -> never arrived
        // 1 -> active(arrived, being traversed currently)
        // 2 -> inactive(arrived, have been traversed)
        self.dfs_flag.extend(std::iter::repeat(0).take(nodes));
        for i in 0..nodes {
            if self.dfs_flag[i] != 0 {
                continue;
            }
            assert!(self.dfs_stack.is_empty());
            self.dfs_stack.push(i);
            'dfs: while !self.dfs_stack.is_empty() {
                let node = *self.dfs_stack.last().unwrap();
                if self.dfs_flag[node] == 0 {
                    // preorder
                    self.dfs_flag[node] = 1;
                }
                for &to in &self.edges[node] {
                    if self.dfs_flag[to] == 0 {
                        self.dfs_stack.push(to);
                        continue 'dfs;
                    }
                }
                // No-op
                let last = self.dfs_stack.pop().unwrap();
                self.temp_index.push(last);
                // Deactivate
                self.dfs_flag[last] = 2;
            }
        }
        self.temp_index.reverse();
        assert!(self.dfs_stack.is_empty());
        // Use DFS stack as a temporary buffer.
        std::mem::swap(&mut self.temp_index, &mut self.dfs_stack);
        // Filling zero so that the boundary would be violated.
        self.temp_index.extend(std::iter::repeat(0).take(nodes));
        for (order, &v) in self.dfs_stack.iter().enumerate() {
            self.temp_index[v] = order;
        }
        // Clear the temporary buffer.
        self.dfs_stack.clear();
    }
    fn calc_thr(&self) -> f64 {
        let ave = self.inner.iter().map(|e| e.0).sum::<f64>() / self.inner.len() as f64;
        if THR_ON {
            ave - THR
        } else {
            0.
        }
    }
    fn calc_thr_weight(&self) -> f64 {
        let (sum, denom) =
            self.inner.iter().fold(
                (0., 0.),
                |(x, y), &(w, _)| if w >= 1. { (x + w, y + 1.) } else { (x, y) },
            );
        let ave = sum / denom;
        if THR_ON {
            ave - THR
        } else {
            0.
        }
    }
    fn get_idx(&mut self, kmer: &[u8], nodes: &mut Vec<Kmer>, thr: f64) -> Option<usize> {
        let x = self.inner.get_mut(to_u64(kmer)).unwrap();
        if x.0 <= thr {
            None
        } else if x.1 != std::usize::MAX {
            Some(x.1)
        } else {
            x.1 = nodes.len();
            nodes.push(Kmer::new(kmer, x.0));
            Some(nodes.len() - 1)
        }
    }
    fn prob(x: f64, scale: f64) -> f64 {
        ((scale * x).exp() + 1.).recip()
    }
    fn get_indices(
        &mut self,
        x: &[u8],
        nodes: &mut Vec<Kmer>,
        thr: f64,
        k: usize,
    ) -> Option<(usize, usize)> {
        let (prev, after) = (&x[..k], &x[1..]);
        let from = self.get_idx(prev, nodes, thr)?;
        let to = self.get_idx(after, nodes, thr)?;
        Some((from, to))
    }
    fn get_idx_exp<R: Rng>(
        &mut self,
        kmer: &[u8],
        nodes: &mut Vec<Kmer>,
        thr: f64,
        rng: &mut R,
        idx: usize,
        scale: f64,
    ) -> Option<usize> {
        match self.inner.get_mut(idx) {
            Some(x) if x.0 < thr && rng.gen_bool(Self::prob(x.0 - thr, scale)) => None,
            Some(x) if x.1 != std::usize::MAX => Some(x.1),
            Some(x) => {
                x.1 = nodes.len();
                nodes.push(Kmer::new(kmer, x.0));
                Some(nodes.len() - 1)
            }
            _ => unreachable!(),
        }
    }
    fn get_indices_exp<R: Rng>(
        &mut self,
        x: &[u8],
        nodes: &mut Vec<Kmer>,
        thr: f64,
        k: usize,
        rng: &mut R,
        from: usize,
        to: usize,
        scale: f64,
    ) -> Option<(usize, usize)> {
        let (prev, next) = (&x[..k], &x[1..]);
        let from = self.get_idx_exp(prev, nodes, thr, rng, from, scale)?;
        let next = self.get_idx_exp(next, nodes, thr, rng, to, scale)?;
        Some((from, next))
    }
    pub fn new() -> Self {
        // let inner = HashMap::default();
        let inner = vec![];
        let is_safe = vec![];
        let temp_index = vec![];
        let edges = vec![];
        let fu = find_union::FindUnion::new(0);
        let dfs_stack = vec![];
        let dfs_flag = vec![];
        let buffer = vec![];
        Self {
            inner,
            is_safe,
            temp_index,
            edges,
            fu,
            dfs_stack,
            dfs_flag,
            buffer,
        }
    }
    #[allow(dead_code)]
    fn select_supported_node(edges: &[Vec<usize>], nodes: usize, is_safe: &[bool]) -> Vec<bool> {
        let mut is_supported = vec![false; nodes];
        for (from, edges) in edges.iter().enumerate() {
            for &to in edges.iter() {
                is_supported[to] |= is_safe[from];
                is_supported[from] |= is_safe[to];
            }
        }
        is_supported
    }
}

#[cfg(test)]
mod tests {
    // #[test]
    // fn shrink() {
    //     // Easy
    //     let number_of_nodes = 8;
    //     let edges_and_labels = vec![
    //         vec![(1, b'a'), (4, b'a')],
    //         vec![(2, b'c')],
    //         vec![(3, b't')],
    //         vec![(7, b'c')],
    //         vec![(5, b'c')],
    //         vec![(6, b't')],
    //         vec![(7, b'c')],
    //         vec![],
    //     ];
    //     let (edges, nodes, merges) = super::Factory::shringk(number_of_nodes, edges_and_labels);
    //     assert_eq!(nodes, 5);
    //     let answer = vec![
    //         vec![(1, b'a')],
    //         vec![(2, b'c')],
    //         vec![(3, b't')],
    //         vec![(4, b'c')],
    //         vec![],
    //     ];
    //     assert_eq!(answer, edges);
    //     let answer = vec![vec![0], vec![1, 4], vec![2, 5], vec![3, 6], vec![7]];
    //     assert_eq!(answer, merges);
    //     // Little hard
    //     let number_of_nodes = 9;
    //     let edges_and_labels = vec![
    //         vec![(1, b'a'), (4, b'a')],
    //         vec![(2, b'c')],
    //         vec![(3, b't')],
    //         vec![(7, b'c')],
    //         vec![(5, b'c')],
    //         vec![(6, b't')],
    //         vec![(7, b'c')],
    //         vec![],
    //         vec![(2, b'c')],
    //     ];
    //     let (edges, nodes, merges) = super::Factory::shringk(number_of_nodes, edges_and_labels);
    //     assert_eq!(nodes, 5);
    //     let answer = vec![
    //         vec![(1, b'a')],
    //         vec![(2, b'c')],
    //         vec![(3, b't')],
    //         vec![(4, b'c')],
    //         vec![],
    //     ];
    //     assert_eq!(answer, edges);
    //     let answer = vec![vec![0], vec![1, 4, 8], vec![2, 5], vec![3, 6], vec![7]];
    //     assert_eq!(answer, merges);
    //     // Hard mode
    // }
    use super::*;
    // #[test]
    // fn tip_cut() {
    //     let edges = vec![
    //         vec![1, 2],
    //         vec![],
    //         vec![],
    //         vec![4],
    //         vec![0, 5],
    //         vec![9],
    //         vec![7],
    //         vec![8],
    //         vec![11],
    //         vec![6, 7, 10],
    //         vec![],
    //         vec![],
    //     ];
    //     let nodes = 12;
    //     assert_eq!(edges.len(), 12);
    //     let dist_to_root = Factory::dist_to_root(&edges, nodes);
    //     let answer = vec![1, 0, 0, 7, 6, 5, 3, 2, 1, 4, 0, 0];
    //     assert_eq!(dist_to_root, answer);
    //     // let is_supported = Factory::cut_tip_inner(&edges, nodes, 1 );
    //     // let answer = vec![
    //     //     false, false, false, true, true, true, true, true, true, true, false, true,
    //     // ];
    //     // assert_eq!(is_supported, answer);
    //     let edges = vec![
    //         vec![1],
    //         vec![2, 3],
    //         vec![],
    //         vec![4],
    //         vec![],
    //         vec![6],
    //         vec![7],
    //         vec![0, 8],
    //         vec![5],
    //     ];
    //     let nodes = 9;
    //     assert_eq!(edges.len(), nodes);
    //     let dist_to_root = Factory::dist_to_root(&edges, nodes);
    //     let answer = vec![3, 2, 0, 1, 0];
    //     eprintln!("{:?}", dist_to_root);
    //     for i in 0..5 {
    //         assert_eq!(dist_to_root[i], answer[i]);
    //     }
    //     for i in 5..nodes {
    //         assert!(dist_to_root[i] > 100);
    //     }
    // let is_supported = Factory::cut_tip_inner(&edges, nodes, 1);
    // let answer = vec![true, true, false, false, false, true, true, true, true];
    // assert_eq!(answer, is_supported);
    // }
    #[test]
    fn select_supported_test() {
        let edges = vec![
            vec![1],
            vec![2],
            vec![3, 7],
            vec![],
            vec![5],
            vec![],
            vec![7],
            vec![4],
            vec![6],
            vec![8, 0],
            vec![9],
        ];
        let nodes = 11;
        let is_safe = vec![
            false, true, true, false, true, true, false, false, false, true, true,
        ];
        let supported = Factory::select_supported_node(&edges, nodes, &is_safe);
        let mut answer = vec![true; nodes];
        answer[6] = false;
        assert_eq!(supported, answer);
    }
    // #[test]
    // fn topological_sorting() {
    //     let edges = vec![
    //         vec![1],
    //         vec![2],
    //         vec![3],
    //         vec![4],
    //         vec![],
    //         vec![6],
    //         vec![2],
    //         vec![8],
    //         vec![9],
    //         vec![3],
    //     ];
    //     let order = Factory::topological_sort_inner(&edges, edges.len()).unwrap();
    //     assert_eq!(order, vec![7, 8, 9, 5, 6, 0, 1, 2, 3, 4]);
    //     let edges = vec![
    //         vec![1],
    //         vec![2],
    //         vec![3],
    //         vec![4, 8],
    //         vec![],
    //         vec![6],
    //         vec![2],
    //         vec![8],
    //         vec![9],
    //         vec![3],
    //     ];
    //     let order = Factory::topological_sort_inner(&edges, edges.len()).unwrap_err();
    //     assert_eq!(order, vec![7, 5, 6, 0, 1, 2, 3, 8, 9, 4]);
    //     let edges = vec![
    //         vec![1],
    //         vec![2, 7],
    //         vec![3],
    //         vec![4],
    //         vec![5],
    //         vec![6],
    //         vec![],
    //         vec![8],
    //         vec![5],
    //     ];
    //     let order = Factory::topological_sort_inner(&edges, edges.len()).unwrap();
    //     assert_eq!(order, vec![0, 1, 7, 8, 2, 3, 4, 5, 6]);
    // }
    #[test]
    fn kmertousize() {
        let xs = b"GGTGTT";
        assert!(to_u64(xs) <= 4096);
    }
    #[test]
    fn decoding() {
        let k = 8 as usize;
        for kmer in 0..(4u64.pow(k as u32)) {
            let kmer = kmer as usize;
            let to_vec = decode(kmer as u64, k);
            let to_int = to_u64(&to_vec);
            let to_vec = decode(to_int as u64, k);
            let to_int = to_u64(&to_vec);
            assert_eq!(to_int, kmer);
        }
    }
    #[test]
    fn limited_dfs() {
        let nodes = 11;
        let mut f = Factory::new();
        f.edges.push(vec![1]);
        f.edges.push(vec![2]);
        f.edges.push(vec![3, 6]);
        f.edges.push(vec![4]);
        f.edges.push(vec![5]);
        f.edges.push(vec![8]);
        f.edges.push(vec![7]);
        f.edges.push(vec![8]);
        f.edges.push(vec![9]);
        f.edges.push(vec![10]);
        f.edges.push(vec![]);
        f.is_safe.extend(std::iter::repeat(0).take(nodes));
        f.dfs_flag.extend(std::iter::repeat(0).take(nodes));
        f.depth_limited_dfs(3, 0, 0);
        assert_eq!(f.is_safe, vec![1, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0]);
        f.depth_limited_dfs(3, 2, 1);
        assert_eq!(f.is_safe, vec![1, 1, 3, 3, 2, 2, 3, 2, 2, 0, 0]);
    }
}
