use std::{ cmp::min, fmt::Write as _ };
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

fn hex_char(half_byte: u8) -> char {
    (half_byte + (if half_byte < 10 { 0x30 } else { 0x41 })) as char
}

fn append_hex_byte(s: &mut String, byte: u8) {
    s.push(hex_char(byte >> 4));
    s.push(hex_char(byte & 0x0F));
}

/*
fn append_hex_bytes(s: &mut String, bytes: &[u8]) {
    s.reserve(bytes.len() * 2);
    for byte in bytes {
        append_hex_byte(s, *byte);
    }
}
*/

struct AsHex<'a> {
    data: &'a [u8],
}

// https://doc.rust-lang.org/std/fmt/struct.Formatter.html#examples-6
impl std::fmt::Display for AsHex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // TODO: reserve f
        let precision = f.precision().unwrap_or(0);
        let width = f.width().unwrap_or(0);

        for (i, byte) in self.data.iter().enumerate() {
            if i > 0 {
                let w = if width > 0 { i % width } else { i };
                if w == 0 {
                    write!(f, "\n")?;
                } else if precision > 0 && w % precision == 0 {
                    write!(f, " ")?;
                }
            }

            // write!(f, "{}{}", hex_char(byte >> 4), hex_char(byte & 0x0F))?;
            write!(f, "{:02X}", byte)?;
        }

        Ok(())
    }
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
        // already checked that file size fits in u32
        let data_len = (data.len() as u32).checked_sub(Self::HEAD_SIZE).context(
            format!("Subrecord contains less than {} bytes", Self::HEAD_SIZE)
        )?;

        let size = parse_u32(&data[4..]);
        if data_len < size {
            return Err(anyhow!(
                "Subrecord size ({}) larger than remaining file size ({})",
                size, data_len
            ));
        }

        Ok(Self {
            start: start,
            size: size,
        })
    }

    fn as_xml(&self, xml: &mut String, data: &[u8]) -> Result<()> {
        let data = &data[self.start as usize..];
        let tag = xml_tag(&data[..4]);

        write!(xml, "  <{} size=\"{}\">\n", tag, self.size)?;

        let data = &data[Self::HEAD_SIZE as usize..];
        write!(xml, "    {:64.8}\n", AsHex { data: &data[..self.size as usize] })?;

        write!(xml, "  </{}>\n", tag)?;

        Ok(())
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
        // already checked that file size fits in u32
        let data_len = (data.len() as u32).checked_sub(Self::HEAD_SIZE).context(
            format!("Record contains less than {} bytes", Self::HEAD_SIZE)
        )?;

        let size = parse_u32(&data[4..]);
        if data_len < size {
            return Err(anyhow!(
                "Record size ({}) larger than remaining file size ({})",
                size, data_len
            ));
        }

        let mut subrecords = Vec::new();
        let mut i: u32 = Self::HEAD_SIZE;
        let end = size + Self::HEAD_SIZE;
        while i < end {
            let subrecord = Subrecord::new(&data[i as usize ..], i).context(
                format!("Subrecord {} at offset {}", subrecords.len(), i)
            )?;
            i += Subrecord::HEAD_SIZE + subrecord.size;
            subrecords.push(subrecord);
        }

        Ok(Self {
            start: start,
            size: size,
            subrecords: subrecords,
        })
    }

    fn as_xml(&self, xml: &mut String, data: &[u8]) -> Result<()> {
        let data = &data[self.start as usize..];
        let tag = xml_tag(&data[..4]);

        write!(xml, "<{} size=\"{}\" flags=\"{}\">\n",
            tag,
            self.size,
            AsHex { data: &data[8..16] }
        )?;

        for (i, subrecord) in self.subrecords.iter().enumerate() {
            subrecord.as_xml(xml, data).context(
                format!("XML formatting subrecord {}", i)
            )?;
        }

        write!(xml, "</{}>\n", tag)?;

        Ok(())
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
                    i += Record::HEAD_SIZE + record.size;
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
            path: path,
            data: data,
            records: records,
        })
    }

    fn as_xml(&self) -> Result<String> {
        let data = &self.data[..];
        let mut xml = String::new();
        for (i, record) in self.records.iter().enumerate() {
            record.as_xml(&mut xml, data).context(
                format!("XML formatting record {}", i)
            )?;
        }
        Ok(xml)
    }
}

const USAGE: &str = "usage: chim input [output]";

// TODO: read from any buffer
// TODO: write to any buffer, including a file

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args.next().context(format!("Input file not specified.\n{USAGE}"))?;

    || -> Result<()> {
        let file = File::new(input.clone())?;
        println!("{:?} {:?}", file.records.len(), file.path);

        print!("{}", file.as_xml()?);

        Ok(())
    }().context(format!("Input file {}", input))?;

    Ok(())
}
