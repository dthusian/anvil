
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, stdout, Write};
use anyhow::{Context, Error, Result};
use atty::Stream;
use clap::{Parser, Subcommand};
use flate2::read::ZlibDecoder;

#[derive(Parser)]
#[command(author, version, about)]
struct Arguments {
  #[command(subcommand)]
  command: Commands
}

#[derive(Subcommand)]
enum Commands {
  Inspect {
    #[arg(short, long, help = "Machine readable output")]
    machine: bool,
    #[arg(short, long, help = "File to read")]
    file: String
  },
  Extract {
    #[arg(short, long, help = "File to read")]
    file: String,
    #[arg(short, long, help = "Chunk ID to extract")]
    chunk: u32
  }
}

#[derive(Debug, Default)]
struct HeaderEntry {
  pub id: u32,
  pub start: u32,
  pub end: u32,
  pub size: u32
}

fn read_header(file: &mut (impl Read + Seek)) -> Result<Vec<HeaderEntry>> {
  let mut header = vec![];
  file.seek(SeekFrom::Start(0))?;
  for id in 0..1024 {
    let mut entry = [0u8; 4];
    file.read(&mut entry)?;
    if entry.iter().all(|v| *v == 0) {
      continue
    }
    let start = u32::from(entry[2]) | (u32::from(entry[1]) << 8) | (u32::from(entry[0]) << 16);
    let size = entry[3] as u32;
    let end = start + size;
    header.push(HeaderEntry {
      id,
      start,
      end,
      size
    })
  }
  Ok(header)
}

fn main() -> Result<()> {
  let args = Arguments::parse();
  match args.command {
    Commands::Inspect { machine, file } => {
      let mut file = File::open(file).context("Failed to open region file")?;
      let header = read_header(&mut file).context("Failed to parse header")?;
      if !machine {
        println!("{:<6}{:<6}{:<6}{:<6}", "ID", "Start", "End", "Size");
      }
      for entry in header {
        println!("{:<6}{:<6}{:<6}{:<6}", entry.id, entry.start, entry.end, entry.size);
      }
    }
    Commands::Extract { file, chunk } => {
      if atty::is(Stream::Stdout) {
        return Err(Error::msg("Refusing to output to a tty (please pipe output to a file)"));
      }
      let mut file = File::open(file).context("Failed to open region file")?;
      let header = read_header(&mut file).context("Failed to parse header")?;
      let found_chunk = header.into_iter().find(|v| v.id == chunk);
      if found_chunk.is_none() {
        return Err(Error::msg("No chunk with that ID"));
      }
      let found_chunk = found_chunk.unwrap();
      file.seek(SeekFrom::Start(u64::from(found_chunk.start * 4096)))?;
      let len;
      {
        let mut buf = [0u8; 4];
        file.read(&mut buf)?;
        len = u32::from_be_bytes(buf);
      }
      let mut fmt = [0u8];
      file.read(&mut fmt)?;
      if fmt[0] != 2 {
        return Err(Error::msg("Unsupported format, the only supported format is 2 (Zlib)"));
      }
      let mut decoder = ZlibDecoder::new(file);
      let mut data = Vec::new();
      decoder.read_to_end(&mut data)?;
      stdout().lock().write(&data)?;
    }
  }
  Ok(())
}
