//! Generate a disposable two-sided tree for the real GUI benchmark.
use std::{fs,path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut args=std::env::args().skip(1);
    let root=PathBuf::from(args.next().ok_or("fixture root required")?);
    let count:usize=args.next().unwrap_or("100000".into()).parse()?;
    let temp=fs::canonicalize(std::env::temp_dir())?;
    if !fs::canonicalize(&root)?.starts_with(temp)||fs::read_dir(&root)?.next().is_some(){return Err("fixture root must be an empty system-temporary directory".into());}
    let started=std::time::Instant::now();
    for i in 0..count{
        for side in ["left","right"]{
            let dir=root.join(side).join(format!("dir{:03}",i/1000));
            if i%1000==0{fs::create_dir_all(&dir)?;}
            fs::write(dir.join(format!("file{i:06}.txt")),b"A\n")?;
        }
    }
    println!("fixture.files_per_side={count}");
    println!("fixture.generate_seconds={:.6}",started.elapsed().as_secs_f64());
    Ok(())
}
