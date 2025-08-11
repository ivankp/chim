use std::{ io::Read as _, fmt::Write as _, cmp::min };
use anyhow::{ Result, Context, anyhow };
use bstr::ByteSlice;

// TODO: efficient [u8] -> [u8; 4] without checks

fn parse_u32(data: &[u8]) -> u32 {
    /*
    let mut x: u32 = 0;
    x += data[3] as u32; x <<= 8;
    x += data[2] as u32; x <<= 8;
    x += data[1] as u32; x <<= 8;
    x += data[0] as u32;
    x
    */
    u32::from_le_bytes(data[..4].try_into().unwrap())
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

struct Bytes<'a>(&'a [u8]);

// https://doc.rust-lang.org/std/fmt/struct.Formatter.html#examples-6
impl std::fmt::Display for Bytes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // TODO: reserve Formatter buffer
        let precision = f.precision().unwrap_or(0);
        let width = f.width().unwrap_or(0);

        for (i, byte) in self.0.iter().enumerate() {
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

fn is_xml_initial_bytes(data: &[u8]) -> bool {
    for byte in data {
        if !b" \t\r\n".contains(byte) {
            return *byte == b'<';
        }
    }
    false
}

fn xml_tag(data: &[u8]) -> String { // TODO: take [u8, 4]
    let mut tag = String::with_capacity(data.len());
    for byte in data {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' => {
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

fn xml_tag_to_bytes(tag: &str) -> Option<[u8; 4]> {
    let tag_bytes = tag.as_bytes();
    match tag_bytes.len() {
       4 => {
           // Some(<[u8; 4]>::try_from(tag_bytes).unwrap())
           Some(tag_bytes.try_into().unwrap())
       }
       8 => {
           let mut bytes = [0u8; 4];
           for (i, c) in tag_bytes.iter().enumerate() {
               let byte: u8 = c - match c {
                   b'0'..b'9' => b'0',
                   b'A'..b'F' => b'A' - 10u8,
                   b'a'..b'f' => b'a' - 10u8,
                   _ => { return None; }
               };
               bytes[i/2] += byte << ((!(i % 2)) << 2);
           }
           Some(bytes)
       }
       _ => None
    }
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
        if size > data_len {
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

    fn from_xml(node: &roxmltree::Node, data: &mut Vec<u8>) -> Result<Self> {
        let tag = node.tag_name().name();
        let size: u32 = node
            // TODO: put this in a generic function
            .attribute("size").context("Record missing size attribute")?
            .parse().context("Failed to parse record size as u32")?;
        // TODO: validate size

        print!("  {} {}\n", tag, size);

        // TODO: needs to be with respect to the start of the Record
        // Or change was is done for binary parsing
        let start = data.len() as u32; // TODO: validate data length

        data.extend(
            xml_tag_to_bytes(tag).context(format!("Unexpected subrecord tag name {}", tag))?
        );
        data.extend(size.to_ne_bytes());

        // TODO: Parse hex data
        data.resize(data.len() + size as usize, 0u8);

        Ok(Self {
            start: start,
            size: size,
        })
    }

    fn range(&self) -> std::ops::Range<usize> {
        self.start as usize .. (self.start + Self::HEAD_SIZE + self.size) as usize
    }

    fn to_xml(&self, xml: &mut String, data: &[u8]) -> Result<()> {
        let data = &data[self.range()];
        let tag = xml_tag(&data[..4]);

        write!(xml, "  <{} size=\"{}\">", tag, self.size)?;

        let data = &data[Self::HEAD_SIZE as usize ..];

        if data.len() > 32 { write!(xml, "\n")?; }
        write!(xml, "{:32.4}", Bytes(data))?;
        if data.len() > 32 { write!(xml, "\n  ")?; }

        write!(xml, "</{}>\n", tag)?;

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
        if size > data_len {
            return Err(anyhow!(
                "Record size ({}) larger than remaining file size ({})",
                size, data_len
            ));
        }

        let mut subrecords = Vec::new();
        let mut offset: u32 = Self::HEAD_SIZE;
        let end = size + Self::HEAD_SIZE;
        while offset < end {
            let subrecord = Subrecord::new(&data[offset as usize ..], offset).context(
                format!("Subrecord {} at offset {}", subrecords.len(), offset)
            )?;
            offset += Subrecord::HEAD_SIZE + subrecord.size;
            subrecords.push(subrecord);
        }

        Ok(Self {
            start: start,
            size: size,
            subrecords: subrecords,
        })
    }

    fn from_xml(node: &roxmltree::Node, data: &mut Vec<u8>) -> Result<Self> {
        let tag = node.tag_name().name();
        let size: u32 = node
            // TODO: put this in a generic function
            .attribute("size").context("Record missing size attribute")?
            .parse().context("Failed to parse record size as u32")?;
        // TODO: validate size

        print!("{} {}\n", tag, size);

        let start = data.len() as u32; // TODO: validate data length

        data.extend(
            xml_tag_to_bytes(tag).context(format!("Unexpected record tag name {}", tag))?
        );
        data.extend(size.to_ne_bytes());
        data.extend([0u8; 8]); // TODO: record flags

        let mut subrecords = Vec::new();
        // TODO: reserve
        for child in node.children() {
            if child.is_element() {
                subrecords.push(Subrecord::from_xml(&child, data)?);
            }
        }

        Ok(Self {
            start: start,
            size: size,
            subrecords: subrecords,
        })
    }

    fn range(&self) -> std::ops::Range<usize> {
        self.start as usize .. (self.start + Self::HEAD_SIZE + self.size) as usize
    }

    fn to_xml(&self, xml: &mut String, data: &[u8]) -> Result<()> {
        let data = &data[self.range()];
        let tag = xml_tag(&data[..4]);

        write!(xml, "<{} size=\"{}\"", tag, self.size)?;

        let flags = &data[8..16];
        if flags.iter().any(|&x| x != 0) {
            write!(xml, " flags=\"{}\"", Bytes(flags))?;
        }

        write!(xml, ">\n")?;

        for (i, subrecord) in self.subrecords.iter().enumerate() {
            subrecord.to_xml(xml, data).context(
                format!("XML formatting subrecord {}", i)
            )?;
        }

        write!(xml, "</{}>\n", tag)?;

        Ok(())
    }
}

pub struct File {
    data: Vec<u8>,
    records: Vec<Record>,
    // path: String,
}

impl File {
    pub fn new(path: &str) -> Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let mut data = Vec::new();
        let size = {
            let size = u32::try_from(file.metadata()?.len())?;
            let read = u32::try_from(file.read_to_end(&mut data)?)?;
            if read == size || size == 0 {
                Ok(read)
            } else {
                Err(anyhow!("Expected {} bytes, read {} bytes", size, read))
            }
        }.context("File size")?;

        if data.starts_with(b"TES3") { // TES3 -----------------------------------------------------
            let mut records = Vec::new();

            let mut offset: u32 = 0;
            while offset < size {
                let record = Record::new(&data[offset as usize ..], offset).context(
                    format!("Record {} at offset {}", records.len(), offset)
                )?;
                if offset == 0 {
                    // TODO: reserve vector of records
                    records.reserve_exact(31);
                    // pub fn try_reserve_exact(
                    //     &mut self,
                    //     additional: usize,
                    // ) -> Result<(), TryReserveError>
                }
                offset += Record::HEAD_SIZE + record.size;
                records.push(record);
            }

            Ok(Self { data, records /*, path: path.to_owned() */ })

        } else if is_xml_initial_bytes(&data[..]) { // XML -----------------------------------------
            let doc = roxmltree::Document::parse(str::from_utf8(&data)?)?;
            let root = doc.root_element();
            println!("root element: {:?}", root.tag_name());

            // Self::from_xml(&root)
            let mut file = Self { data: Vec::new(), records: Vec::new() };
            // TODO: reserve
            for child in root.children() {
                if child.is_element() {
                    file.records.push(Record::from_xml(&child, &mut file.data)?);
                }
            }
            Ok(file)

        } else { // --------------------------------------------------------------------------------
            Err(anyhow!(
                "Unexpected initial bytes: {:?}",
                &data[..min(4,data.len())].as_bstr()
            ))
        }
    }

    pub fn to_xml(&self) -> Result<String> {
        let data = &self.data[..];
        let mut xml = String::new();
        write!(&mut xml, "<CHIM>\n")?;
        for (i, record) in self.records.iter().enumerate() {
            record.to_xml(&mut xml, data).context(
                format!("XML formatting record {}", i)
            )?;
        }
        write!(&mut xml, "</CHIM>\n")?;
        Ok(xml)
    }

    // fn from_xml(node: &roxmltree::Node) -> Result<Self> {
    //     let mut file = Self { data: Vec::new(), records: Vec::new() };
    //     for child in node.children() {
    //         if child.is_element() {
    //             Record::from_xml(&mut file, &child)?;
    //         }
    //     }
    //     Ok(file)
    // }
}

// TODO: read from any buffer
// TODO: write to any buffer, including a file
