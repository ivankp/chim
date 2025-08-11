use anyhow::{ Result, Context };

const USAGE: &str = "usage: chim input [output]";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args.next().context(format!("Input file not specified.\n{USAGE}"))?;

    {
        let file = chim::File::new(&input)?;
        // println!("{:?} {:?}", file.records.len(), file.path);

        print!("{}", file.to_xml()?);

        anyhow::Ok(())
    }.context(format!("Input file {}", input))?;

    Ok(())
}
