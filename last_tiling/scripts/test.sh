#!/bin/bash
#$ -S /bin/bash
#$ -N Convert
#$ -cwd
#$ -pe smp 12
#$ -e ./logfiles/convert.log
#$ -o ./logfiles/convert.out
#$ -V
set -ue
ROOT=/grid/ban-m/arabidopsis_thaliana/sequel/assemble/mine
LASTTAB=${ROOT}/last_db/collupsed.tab
READ=${ROOT}/filtered_read.fasta
CONTIGS=${ROOT}/contigs.fasta
SELF_LASTTAB=${ROOT}/last_db/self_no_repeat.tab
PREFIX=./data/test
cargo run --release --bin annotate_dotplot -- ${CONTIGS} ${SELF_LASTTAB} \
      > ${PREFIX}_repeats.json
cargo run --release --bin main \
      -- ${LASTTAB} ${CONTIGS} ${READ} ${PREFIX}_repeats.json 
