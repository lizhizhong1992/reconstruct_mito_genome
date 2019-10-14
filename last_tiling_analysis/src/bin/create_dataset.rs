extern crate last_tiling;
extern crate bio_utils;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate serde;
extern crate serde_json;
use env_logger::Env;
use std::io::{BufWriter, Write};
fn main() -> std::io::Result<()> {
    env_logger::from_env(Env::default().default_filter_or("debug")).init();
    // env_logger::from_env(Env::default().default_filter_or("warn")).init();
    let args: Vec<_> = std::env::args().collect();
    info!("Start");
    let alignments = last_tiling::parse_tab_file(&args[1])?;
    debug!("Alignments:{}", alignments.len());
    let peaks = last_tiling::parse_peak_file(&args[2], &args[3])?;
    debug!("\nPeak call files:{}", peaks);
    let fasta = bio_utils::fasta::parse_into_vec(&args[4])?;
    debug!("Read num\t{}", fasta.len());
    let encoded_reads = last_tiling::encoding(&fasta, &peaks, &alignments);
    debug!("Encoded:\t{}", encoded_reads.len());
    let out = std::io::stdout();
    let mut out = BufWriter::new(out.lock());
    for read in &encoded_reads {
        writeln!(&mut out, "{}", read)?;
    }
    eprintln!("Output dump");
    let mut wtr = std::fs::File::create("./data/peaks.json")?;
    wtr.write_all(serde_json::ser::to_string_pretty(&peaks)?.as_bytes())?;
    let mut wtr = std::fs::File::create("./data/reads.json")?;
    wtr.write_all(serde_json::ser::to_string(&encoded_reads)?.as_bytes())
}