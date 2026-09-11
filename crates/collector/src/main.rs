use collector::collect_reader;
use std::io;

fn main() -> io::Result<()> {
    collect_reader(io::stdin().lock(), io::BufWriter::new(io::stdout().lock())).map(|_| ())
}
