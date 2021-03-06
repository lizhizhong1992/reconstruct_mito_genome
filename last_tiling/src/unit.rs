//! A module to represent encoded reads.
use super::lasttab;
use std::fmt;

/// A struct to represent encoded read.
/// It should be used with the corresponding UnitDefinitions.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EncodedRead {
    pub id: String,
    pub seq: Vec<ChunkedUnit>,
    pub desc: Option<String>,
}

use std::hash::{Hash, Hasher};

impl Hash for EncodedRead {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for EncodedRead {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl Eq for EncodedRead {}

impl EncodedRead {
    pub fn from(id: String, seq: Vec<ChunkedUnit>, desc: Option<String>) -> Self {
        Self { id, seq, desc }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn seq(&self) -> &[ChunkedUnit] {
        &self.seq
    }
    pub fn desc(&self) -> Option<&String> {
        self.desc.as_ref()
    }
    pub fn recover_raw_sequence(&self) -> Vec<u8> {
        self.seq
            .iter()
            .flat_map(|e| e.recover_raw_sequence())
            .collect()
    }
    pub fn has(&self, contig: u16) -> bool {
        self.seq.iter().any(|e| match e {
            ChunkedUnit::En(ref e) => e.contig == contig,
            _ => false,
        })
    }
    /// Determine the direction of this read with respect to given contig.
    /// Note that it can be happen that this read is forward wrt contig0,
    /// while reverse wrt contig1.(contig representation is ambiguous).
    /// If there is no unit with contig, return None.
    pub fn is_forward_wrt(&self, contig: u16) -> Option<bool> {
        let (tot, forward) = self
            .seq
            .iter()
            .filter_map(|e| e.encode())
            .filter(|encode| encode.contig == contig)
            .map(|encode| encode.is_forward())
            .fold((0, 0), |(tot, forward), b| {
                if b {
                    (tot + 1, forward + 1)
                } else {
                    (tot + 1, forward)
                }
            });
        if tot == 0 {
            None
        } else {
            Some(2 * forward > tot)
        }
    }
}

impl fmt::Display for EncodedRead {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, ">{}", self.id)?;
        for unit in &self.seq {
            write!(f, "{} ", unit)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkedUnit {
    En(Encode),
    Gap(GapUnit),
}

impl ChunkedUnit {
    pub fn is_gap(&self) -> bool {
        match self {
            ChunkedUnit::En(_) => false,
            ChunkedUnit::Gap(_) => true,
        }
    }
    pub fn is_encode(&self) -> bool {
        match self {
            ChunkedUnit::En(_) => true,
            ChunkedUnit::Gap(_) => false,
        }
    }
    pub fn encode(&self) -> Option<&Encode> {
        match self {
            ChunkedUnit::En(res) => Some(res),
            ChunkedUnit::Gap(_) => None,
        }
    }
    pub fn gap(&self) -> Option<&GapUnit> {
        match self {
            ChunkedUnit::En(_) => None,
            ChunkedUnit::Gap(gap) => Some(gap),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            ChunkedUnit::En(e) => e.len(),
            ChunkedUnit::Gap(e) => e.len(),
        }
    }
    pub fn is_empty(&self)->bool{
        self.len() == 0
    }
    pub fn recover_raw_sequence(&self) -> Vec<u8> {
        match self {
            ChunkedUnit::En(e) => e.bases.as_bytes().to_vec(),
            ChunkedUnit::Gap(e) => e.bases.as_bytes().to_vec(),
        }
    }
}

impl fmt::Display for ChunkedUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::En(encode) => write!(f, "Encode({})", encode),
            Self::Gap(gap) => write!(f, "Gap({})", gap),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GapUnit {
    contig_pair: Option<(u16, u16)>,
    bases: String,
}

impl fmt::Display for GapUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some((s, e)) = self.contig_pair {
            write!(f, "{}->[{}]->{}", s, &self.bases.len(), e)
        } else {
            write!(f, "[{}]", &self.bases.len())
        }
    }
}

impl fmt::Debug for GapUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some((s, e)) = self.contig_pair {
            write!(f, "{}->[{}]->{}", s, &self.bases, e)
        } else {
            write!(f, "[{}]", &self.bases)
        }
    }
}

impl GapUnit {
    pub fn new(seq: &[u8], contig_pair: Option<(u16, u16)>) -> Self {
        let bases = String::from_utf8(seq.to_vec()).unwrap();
        Self { bases, contig_pair }
    }
    pub fn len(&self) -> usize {
        self.bases.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bases.is_empty()
    }
    pub fn set_bases(&mut self, seq: &[u8]) {
        self.bases.clear();
        self.bases.push_str(&String::from_utf8_lossy(seq));
    }
    pub fn bases(&self) -> &[u8] {
        self.bases.as_bytes()
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Hash)]
pub struct Encode {
    pub contig: u16,
    pub unit: u16,
    pub bases: String,
    pub ops: Vec<lasttab::Op>,
    pub is_forward: bool,
}

impl fmt::Display for Encode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let t = if self.is_forward { 'F' } else { 'R' };
        write!(f, "{}:{}({})", self.contig, self.unit, t)
    }
}

impl Encode {
    pub fn sketch(contig: u16, unit: u16, is_forward: bool) -> Self {
        let bases = String::new();
        let ops = vec![];
        Self {
            contig,
            unit,
            bases,
            ops,
            is_forward,
        }
    }
    pub fn len(&self) -> usize {
        self.bases.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bases.is_empty()
    }
    pub fn set_bases(&mut self, seq: &[u8]) {
        self.bases.clear();
        self.bases.push_str(&String::from_utf8_lossy(seq));
    }
    pub fn set_ops(&mut self, ops: &[lasttab::Op]) {
        self.ops.clear();
        self.ops.extend(ops);
    }
    pub fn is_forward(&self) -> bool {
        self.is_forward
    }
    // The reference should be consistent with the `is_forward` value.
    pub fn view(&self, refr: &[u8]) {
        let (mut r, mut q) = (0, 0);
        let bases = self.bases.as_bytes();
        let (mut rs, mut qs) = (vec![], vec![]);
        for op in &self.ops {
            match op {
                lasttab::Op::Match(l) => {
                    rs.extend(&refr[r..r + l]);
                    qs.extend(&bases[q..q + l]);
                    r += l;
                    q += l;
                }
                lasttab::Op::Seq1In(l) => {
                    rs.extend(&vec![b'-'; *l]);
                    qs.extend(&bases[q..q + l]);
                    q += l;
                }
                lasttab::Op::Seq2In(l) => {
                    rs.extend(&refr[r..r + l]);
                    qs.extend(&vec![b'-'; *l]);
                    r += l;
                }
            }
        }
        println!("{}", String::from_utf8_lossy(&rs[..100]));
        println!("{}", String::from_utf8_lossy(&qs[..100]));
        println!("{}", String::from_utf8_lossy(&rs[100..]));
        println!("{}", String::from_utf8_lossy(&qs[100..]));
    }
}
