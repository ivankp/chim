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
    fn new(path: String) -> Self {
        let data = std::fs::read(&path).unwrap();
        let size = data.len() as u32; // TODO: check that it fits

        let records = match &data[0..4] {
            b"TES3" => {
                let mut records = Vec::new();
                let mut i: u32 = 0;
                while i < size {
                    let record = Record::new(&data[i as usize ..], i);
                    if i == 0 {
                        records.reserve_exact(31); // TODO
                    }
                    i += record.size;
                    // if i > size
                    records.push(record); // TODO: can I create the object in-place?
                }
                records
            }
            _ => todo!()
        };

        Self {
            path: path,
            data: data,
            records: records,
        }
    }
}

fn usage() -> ! {
    println!("usage: chim input [output]");
    std::process::exit(1);
}

fn main() {
    let mut args = std::env::args().skip(1);

    let input = match args.next() {
        Some(path) => path,
        None => usage(),
    };

    let file = File::new(input);
    println!("{:?} {:?}", file.records.len(), file.path);
}
