extern crate bio_utils;
extern crate last_tiling;
#[macro_use]
extern crate log;
extern crate env_logger;
use env_logger::Env;
fn main() -> std::io::Result<()> {
    env_logger::from_env(Env::default().default_filter_or("debug")).init();
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
    for read in encoded_reads {
        for unit in &read.seq {
            match unit {
                last_tiling::unit::ChunkedUnit::En(encode) => {
                    let peak = peaks.search_unit(encode.contig, encode.unit).unwrap();
                    let pulled = peaks.pull_unit(&peak).unwrap();
                    let refr = &pulled[last_tiling::SUBUNIT_SIZE * encode.subunit as usize
                        ..last_tiling::SUBUNIT_SIZE * (encode.subunit + 1) as usize];
                    encode.view(refr);
                    println!();
                }

                last_tiling::unit::ChunkedUnit::Gap(gap) => {
                    debug!("{:?}", gap);
                }
            }
        }
    }
    Ok(())
}