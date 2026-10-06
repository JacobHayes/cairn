//! Gap 5: one coverage oracle whose label holds a space, reached on every run. The run
//! passes; a campaign over it cannot read the run's SDK report.

fn main() {
    patina_dst::reachable!("a spaced label");
    patina_dst::verdict(patina_dst::VerdictKind::Pass, "spaced-label", "reached");
}
