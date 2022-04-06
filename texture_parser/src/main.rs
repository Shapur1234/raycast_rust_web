use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let im = image::open(&Path::new("textures/richardo.jpeg")).unwrap();
    // println!("{:?}", im.as_bytes().len());

    let mut file = File::create("out.txt")?;
    file.write_all(format!("length: {:?}", im.as_bytes().len()).as_bytes())?;
    file.write_all(format!("vec!{:?};", im.as_bytes()).as_bytes())?;
    Ok(())
}
