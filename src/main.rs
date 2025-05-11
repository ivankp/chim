use anyhow::{Context, Result};

fn as_u32(data: &[u8]) -> u32 {
    let mut x: u32 = 0;
    x += data[3] as u32; x <<= 8;
    x += data[2] as u32; x <<= 8;
    x += data[1] as u32; x <<= 8;
    x += data[0] as u32;
    x
}

// fn convert<T>(data: &[u8]) -> T {
//     let mut buf = [0u8; std::mem::size_of::<T>()];
//     buf.copy_from_slice(data);
//     T::from_be_bytes(buf)
// }

struct SubRecord {
    start: u32,
    size: u32,
}

impl SubRecord {
    fn new(data: &[u8], start: u32) -> Self {
        let size = as_u32(&data[4..]) + 8;
        Self {
            start: start,
            size: size,
        }
    }
}

struct Record {
    start: u32,
    size: u32,
    subrecords: Vec<SubRecord>,
}

impl Record {
    fn new(data: &[u8], start: u32) -> Self {
        let size = as_u32(&data[4..]) + 16;
        let mut subrecords = Vec::new();
        let mut i: u32 = 16;
        while i < size {
            let subrecord = SubRecord::new(&data[i as usize ..], i);
            i += subrecord.size;
            // if i > size
            subrecords.push(subrecord); // TODO: can I create the object in-place?
        }
        Self {
            start: start,
            size: size,
            subrecords: subrecords,
        }
    }
}

struct File {
    path: String,
    data: Vec<u8>,
    records: Vec<Record>,
}

impl File {
    fn new(path: String) -> Result<Self> {
        let data = std::fs::read(&path)?;
        let size = data.len();
        let size = u32::try_from(size).context(
            format!("File size ({}) is too large for a 32 bit unsigned value", size)
        )?;

        let records = match &data[0..4] {
            b"TES3" => {
                let mut records = Vec::new();
                let mut i: u32 = 0;
                while i < size {
                    let record = Record::new(&data[i as usize ..], i);
                    if i == 0 {
                        records.reserve_exact(31); // TODO
                        // pub fn try_reserve_exact(
                        //     &mut self,
                        //     additional: usize,
                        // ) -> Result<(), TryReserveError>
                    }
                    i += record.size;
                    // if i > size
                    records.push(record); // TODO: can I create the object in-place?
                }
                records
            }
            _ => todo!()
        };

        Ok(Self {
            path: path,
            data: data,
            records: records,
        })
    }
}

const USAGE: &str = "usage: chim input [output]";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    let input = args.next().context(format!("Input file not specified.\n{USAGE}"))?;
    let input_clone = input.clone(); // TODO: how to move only if Ok?

    let file = File::new(input).context(format!("Input file {}", input_clone))?;
    println!("{:?} {:?}", file.records.len(), file.path);

    Ok(())
}
