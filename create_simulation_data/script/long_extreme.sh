#!/bin/bash
#$ -S /bin/bash
#$ -N MockGenomePred
#$ -cwd
#$ -pe smp 24
#$ -e ./logfiles/mock_genome_long_extreme.log
#$ -o ./logfiles/mock_genome_long_extreme.log
#$ -V
#$ -m e

ROOT=${PWD}
READ=${ROOT}/data/long_extreme/reads.fa
REFERENCE=${ROOT}/data/long_extreme/mock_genome_ref.fa
OUTPUT=${ROOT}/result/long_extreme/
CORES=24

# ----- Alignment -----
mkdir -p ${OUTPUT}/last_db
cd ${OUTPUT}/last_db
lastdb -R00 -Q0 reference ${REFERENCE}
last-train -P${CORES} -Q0 reference ${READ} > score.matrix
lastal -f tab -P${CORES} -R00 -Q0 -p score.matrix reference ${READ} > alignments.tab
cd ${ROOT}

# ----- Prediction ------
# RUSTFLAGS="-C target-cpu=native" cargo build --release 
./target/release/predict_mockdata \
    ${READ} ${REFERENCE} ${OUTPUT}/last_db/alignments.tab random\
    > ${OUTPUT}/predictions_random.tsv
