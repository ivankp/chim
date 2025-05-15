use std::{ cmp::min };
use anyhow::{ Context, Result, anyhow };
use bstr::ByteSlice;

fn parse_u32(data: &[u8]) -> u32 {
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

/*
fn ascii_or_hex(data: &[u8]) -> String {
    let mut s = String::new();
    s.reserve_exact(data.len() * 2);
    for byte in data {
        if 0x1Fu8 < *byte && *byte < 0x7Fu8 {
            s.push(' ');
            s.push(*byte as char);
        } else {
            append_hex_byte(&mut s, *byte);
        }
    }
    s
}
*/

fn append_hex_byte(s: &mut String, byte: u8) {
    let a = byte >> 4;
    s.push((a + (if a < 10 { 0x30 } else { 0x41 })) as char);
    let a = byte & 0x0F;
    s.push((a + (if a < 10 { 0x30 } else { 0x41 })) as char);
}

fn xml_tag(data: &[u8]) -> String { // TODO: take [u8, 4]
    let mut tag = String::with_capacity(data.len());
    for byte in data {
        match byte {
            0x41u8..0x5Bu8 | 0x61u8..0x7Bu8 | 0x30u8..0x3Au8 | 0x5Fu8 | 0x2Du8 | 0x2Eu8 => {
                tag.push(*byte as char);
            }
            _ => {
                tag.clear();
                tag.reserve_exact(data.len() * 2);
                for byte in data {
                    append_hex_byte(&mut tag, *byte);
                }
                break;
            }
        }
    }
    tag
}

struct Subrecord {
    start: u32,
    size: u32,
}

impl Subrecord {
    const HEAD_SIZE: u32 = 8;

    fn new(data: &[u8], start: u32) -> Result<Self> {
        let data_len = data.len() as u32; // safe, already checked that file size fits in u32
        if data_len < Self::HEAD_SIZE {
            return Err(anyhow!("Subrecord contains less than {} bytes", Self::HEAD_SIZE));
        }
        let size = parse_u32(&data[4..]);
        if data_len - Self::HEAD_SIZE < size {
            return Err(anyhow!(
                "Subrecord size ({}) larger than remaining file size ({})",
                size, data_len - Self::HEAD_SIZE
            ));
        }
        let size = size + Self::HEAD_SIZE;

        Ok(Self {
            start: start,
            size: size,
        })
    }

    fn as_xml(&self, data: &[u8]) -> String {
        let data = &data[self.start as usize..];
        let tag = xml_tag(&data[..4]);
        format!("  <{0}></{0}>\n", tag)
    }
}

struct Record {
    start: u32,
    size: u32,
    subrecords: Vec<Subrecord>,
}

impl Record {
    const HEAD_SIZE: u32 = 16;

    fn new(data: &[u8], start: u32) -> Result<Self> {
        let data_len = data.len() as u32; // safe, already checked that file size fits in u32
        if data_len < Self::HEAD_SIZE {
            return Err(anyhow!("Record contains less than {} bytes", Self::HEAD_SIZE));
        }
        let size = parse_u32(&data[4..]);
        if data_len - Self::HEAD_SIZE < size {
            return Err(anyhow!(
                "Record size ({}) larger than remaining file size ({})",
                size, data_len - Self::HEAD_SIZE
            ));
        }
        let size = size + Self::HEAD_SIZE;

        let mut subrecords = Vec::new();
        let mut i: u32 = Self::HEAD_SIZE;
        while i < size {
            let subrecord = Subrecord::new(&data[i as usize ..], i).context(
                format!("Subrecord {} at offset {}", subrecords.len(), i)
            )?;
            i += subrecord.size;
            subrecords.push(subrecord);
        }
        Ok(Self {
            start: start,
            size: size,
            subrecords: subrecords,
        })
    }

    fn as_xml(&self, data: &[u8]) -> String {
        let data = &data[self.start as usize..];
        let tag = xml_tag(&data[..4]);
        let mut xml = String::new();
        xml += &format!("<{}>\n", tag);
        for subrecord in &self.subrecords {
            xml += &subrecord.as_xml(data);
        }
        xml += &format!("</{}>\n", tag);
        xml
    }
}

struct File {
    path: String,
    data: Vec<u8>,
    records: Vec<Record>,
}

impl File {
    fn new(path: &String) -> Result<Self> {
        let data = std::fs::read(path)?;
        let size = data.len();
        let size = u32::try_from(size).context(
            format!("File size ({}) does not fit into a 32 bit unsigned value", size)
        )?;

        let records = {
            if data.starts_with(b"TES3") {
                let mut records = Vec::new();
                let mut i: u32 = 0;
                while i < size {
                    let record = Record::new(&data[i as usize ..], i).context(
                        format!("Record {} at offset {}", records.len(), i)
                    )?;
                    if i == 0 {
                        // TODO: reserve vector of records
                        records.reserve_exact(31);
                        // pub fn try_reserve_exact(
                        //     &mut self,
                        //     additional: usize,
                        // ) -> Result<(), TryReserveError>
                    }
                    i += record.size;
                    records.push(record);
                }
                Ok(records)
            } else {
                Err(anyhow!(
                    "Unexpected initial bytes: {:?}",
                    &data[..min(4,data.len())].as_bstr()
                ))
            }
        }?;

        Ok(Self {
            path: path.clone(),
            data: data,
            records: records,
        })
    }

    fn as_xml(&self) -> String {
        let data = &self.data[..];
        let mut xml = String::new();
        for record in &self.records {
            xml += &record.as_xml(data);
        }
        xml
    }
}

const USAGE: &str = "usage: chim input [output]";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    let input = args.next().context(format!("Input file not specified.\n{USAGE}"))?;

    let file = File::new(&input).context(format!("Input file {}", input))?;
    println!("{:?} {:?}", file.records.len(), file.path);

    print!("{}", file.as_xml());

    Ok(())
}
