extern crate dbg_hmm;
extern crate edlib_sys;
extern crate env_logger;
extern crate rand;
extern crate rayon;
use dbg_hmm::gen_sample::*;
use dbg_hmm::*;
use rand::{rngs::StdRng, SeedableRng};
use rayon::prelude::*;
fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let len = 50;
    let num_seq = 200;
    let mut rng: StdRng = SeedableRng::seed_from_u64(121_892);
    let k = 6;
    println!("Seed\tCoverage\tNumNode\tLK");
    let rep = 15;
    let covs: Vec<_> = (1..num_seq).collect();
    let result: Vec<_> = (0..rep)
        .flat_map(|seed| {
            let template: Vec<_> = generate_seq(&mut rng, len);
            let data: Vec<Vec<_>> = (0..num_seq)
                .map(|_| introduce_randomness(&template, &mut rng, &PROFILE))
                .collect();
            let tests: Vec<_> = (0..100)
                .map(|_| introduce_randomness(&template, &mut rng, &PROFILE))
                .collect();
            let mut f = Factory::new();
            let mut result = vec![];
            for &cov in &covs {
                let m: Vec<_> = data[..cov].iter().map(|e| e.as_slice()).collect();
                let w = vec![1.; cov];
                let m = f.generate_with_weight_prior(&m, &w, k, &mut vec![]);
                let n = m.node_num();
                let samples = tests
                    .par_iter()
                    .map(|q| {
                        let lk = m.forward(&q, &DEFAULT_CONFIG);
                        (seed, cov, n, lk)
                    })
                    .collect::<Vec<_>>();
                result.extend(samples);
            }
            result
        })
        .collect();
    for (seed, cov, ratio, orig) in result {
        println!("{}\t{}\t{}\t{}", seed, cov, ratio, orig);
    }
}
