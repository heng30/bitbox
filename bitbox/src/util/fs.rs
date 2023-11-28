use anyhow::Result;
use std::path::PathBuf;
use std::env;

pub fn working_dir() -> Result<PathBuf> {
    let mut dir = env::current_exe()?;
    dir.pop();

    match dir.to_str() {
        Some(path) => Ok(PathBuf::from(path)),
        _ => Err(anyhow::anyhow!("convert {:?} failed", dir)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_dir() -> Result<()>{
        let wd = working_dir()?;
        // println!("{:?}", wd);
        assert!(wd.is_dir());

        Ok(())
    }
}
