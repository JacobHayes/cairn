//! Gap 6: an `always!` that never holds. The run reports its violation verdict and aborts,
//! and a recording of it is left unfinished, so it cannot be replayed.

fn main() {
    let arguments = std::env::args().count();
    patina_dst::always!(arguments > 64, "never-holds");
}
