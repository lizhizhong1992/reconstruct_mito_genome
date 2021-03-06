extern crate last_tiling;
#[macro_use]
extern crate serde;
extern crate env_logger;
extern crate last_decompose;
extern crate log;
extern crate serde_json;
use last_decompose::find_breakpoint::ReadClassify;
use last_decompose::Cluster;
use last_tiling::EncodedRead;
use std::io::{BufReader, BufWriter};
fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let args: Vec<_> = std::env::args().collect();
    let contigs: last_tiling::Contigs =
        serde_json::de::from_reader(std::fs::File::open(&args[1]).map(BufReader::new)?).unwrap();
    let reads: Vec<last_tiling::EncodedRead> =
        serde_json::de::from_reader(std::fs::File::open(&args[2]).map(BufReader::new)?).unwrap();
    let clusters = last_decompose::initial_clusters(&reads, &contigs);

    let contigs = summarize_contig(&contigs, &reads);
    let reads = summarize_reads(&reads, &clusters);
    let summary = Summary {
        contigs,
        reads,
        clusters,
    };
    let stdout = std::io::stdout();
    let mut stdout = BufWriter::new(stdout.lock());
    serde_json::ser::to_writer(&mut stdout, &summary).unwrap();
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Summary {
    contigs: Vec<Contig>,
    reads: Vec<Read>,
    clusters: Vec<Cluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Contig {
    name: String,
    id: u16,
    length: usize,
    coverages: Vec<u32>,
    start_stop: Vec<u32>,
}

fn summarize_contig(
    contigs: &last_tiling::Contigs,
    reads: &[last_tiling::EncodedRead],
) -> Vec<Contig> {
    let mut cs: Vec<_> = contigs
        .names()
        .iter()
        .enumerate()
        .map(|(id, name)| {
            let id = id as u16;
            let length = contigs.get(name).unwrap().len();
            let coverages = vec![0; length / last_tiling::UNIT_SIZE + 1];
            let start_stop = vec![0; length / last_tiling::UNIT_SIZE + 1];
            let name = name.to_string();
            Contig {
                id,
                length,
                coverages,
                name,
                start_stop,
            }
        })
        .collect();
    for read in reads {
        let mut first = true;
        for unit in &read.seq {
            if let last_tiling::unit::ChunkedUnit::En(encode) = unit {
                if first {
                    cs[encode.contig as usize].start_stop[encode.unit as usize] += 1;
                    first = false;
                }
                cs[encode.contig as usize].coverages[encode.unit as usize] += 1
            }
        }
        if let Some(last_tiling::unit::ChunkedUnit::En(encode)) =
            &read.seq.iter().rev().find(|e| e.is_encode())
        {
            cs[encode.contig as usize].start_stop[encode.unit as usize] += 1;
        }
    }
    cs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Read {
    name: String,
    units: Vec<Unit>,
    cluster: i32,
}

impl Read {
    fn from(read: &EncodedRead, cr: &[Cluster]) -> Self {
        let name = String::from(read.id());
        let cluster = get_cluster(read, cr);
        let mut units = vec![];
        let mut start = std::u16::MAX;
        let (mut prev_c, mut prev_u) = (std::u16::MAX, std::u16::MAX);
        for unit in read.seq().iter() {
            if unit.is_encode() {
                let u = unit.encode().unwrap();
                let diff = if u.unit < prev_u {
                    prev_u - u.unit
                } else {
                    u.unit - prev_u
                };
                if u.contig == prev_c && diff < 2 {
                    prev_u = u.unit;
                } else {
                    //If not right after a gap.
                    if prev_c != std::u16::MAX {
                        let e = Unit::E(prev_c, start, prev_c, prev_u);
                        units.push(e);
                    }
                    start = u.unit;
                    prev_u = u.unit;
                    prev_c = u.contig;
                }
            } else {
                // If not the head.
                if prev_c != std::u16::MAX {
                    let e = Unit::E(prev_c, start, prev_c, prev_u);
                    units.push(e);
                }
                prev_c = std::u16::MAX;
                prev_u = std::u16::MAX;
                units.push(Unit::G(unit.gap().unwrap().len()));
            }
        }
        if prev_c != std::u16::MAX && prev_u != std::u16::MAX {
            let e = Unit::E(prev_c, start, prev_c, prev_u);
            units.push(e);
        }
        Self {
            name,
            cluster,
            units,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
enum Unit {
    // The size of the gap
    G(usize),
    // Start contig,Start unit, end contig, end unit.
    E(u16, u16, u16, u16),
}

fn summarize_reads(reads: &[last_tiling::EncodedRead], clusters: &[Cluster]) -> Vec<Read> {
    reads.iter().map(|r| Read::from(r, clusters)).collect()
}

fn get_cluster(read: &EncodedRead, clusters: &[Cluster]) -> i32 {
    clusters
        .iter()
        .filter(|cluster| cluster.has(read.id()))
        .map(|cluster| cluster.id as i32)
        .next()
        .unwrap_or(-1)
}
