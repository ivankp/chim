// use std::mem::size_of;

// struct Record {
//     data: &[u8],
//     //    ^ expected named lifetime parameter
// }

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

fn parse_subrecords(mut data: &[u8]) -> Vec<&[u8]> {
    let mut records = Vec::new();
    while !data.is_empty() {
        let (head, tail) = data.split_at(as_u32(&data[4..8]) as usize + 8);
        records.push(head);
        data = tail;
    }
    records
}

fn parse_records(mut data: &[u8]) -> Vec<(&[u8], Vec<&[u8]>)> {
    let mut records = Vec::new();
    while !data.is_empty() {
        let (head, tail) = data.split_at(as_u32(&data[4..8]) as usize + 16);
        records.push((head, parse_subrecords(&head[16..])));
        data = tail;
    }
    records
}

fn parse_tes3(data: &[u8]) {
    println!("Morrowind");
    let records = parse_records(data);
    println!("Num records: {:?}", records.len());
}

fn read_file(path: &String) -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read(path)?;
    println!("{:?}", data.len());

    match &data[0..4] {
        b"TES3" => parse_tes3(&data[..]),
        b"TES4" => {
            println!("Oblivion");
        }
        _ => {
            println!("Unexpected file head bytes");
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    println!("{:?}", args);

    read_file(&args[1]).unwrap();
}
