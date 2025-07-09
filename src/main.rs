use clap::{Parser, ValueEnum};
use libpolisim::loader::load_congress_graph_from_toml;
use libpolisim::sim::{Majority, Simulator, gen_random_proposal};
use nalgebra::DVector;

/// Simple CLI for running congressional simulations.
#[derive(Parser)]
#[command(name = "polisim-cli")]
#[command(version = "0.1.0")]
#[command(about = "Run a legislative simulation using libpolisim", long_about = None)]
struct Cli {
    /// Path to the TOML config describing members, parties, and edges
    #[arg(short, long)]
    config: String,

    /// Number of rounds to simulate social influence
    #[arg(long, default_value_t = 5)]
    rounds: usize,

    /// Threshold for final vote decision (positive value)
    #[arg(short, long, default_value_t = 0.1)]
    threshold: f64,

    /// Maximum absolute value for random proposal vector entries,
    /// Should be the same as "ideal_dimension" field you declared in toml.
    #[arg(long)]
    range: f64,

    /// Majority rule to decide if the proposal passes
    #[arg(short, long, value_enum, default_value_t = Rule::Simple)]
    rule: Rule,
}

/// We map our internal Majority enum to clap-friendly variants
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum Rule {
    Simple,
    Super,
    AbsSimple,
    AbsSuper,
    Unanimity,
}

impl From<Rule> for Majority {
    fn from(r: Rule) -> Self {
        match r {
            Rule::Simple => Majority::SIMPLE,
            Rule::Super => Majority::SUPER,
            Rule::AbsSimple => Majority::ABSSIMPLE,
            Rule::AbsSuper => Majority::ABSSUPER,
            Rule::Unanimity => Majority::UNANIMITY,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut congress = load_congress_graph_from_toml(&cli.config)
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;

    let dim = congress
        .graph
        .node_weights()
        .next()
        .map(|n| n.ideal.len())
        .ok_or_else(|| anyhow::anyhow!("No members in graph"))?;

    let proposal: DVector<f64> = gen_random_proposal(dim, cli.range);
    println!("Using random proposal: {}", proposal);

    let mut sim = Simulator::new(&congress, proposal);
    sim.run(cli.rounds, cli.threshold);

    println!("\nFinal votes:");
    for (id, vote) in sim.get_votes().iter() {
        let sign = match vote {
            1 => "YES",
            0 => "ABSTAIN",
            -1 => "NO",
            _ => unreachable!(),
        };
        println!("  {:<15} → {}", id, sign);
    }

    let passed = sim.passes(cli.rule.into());
    println!(
        "\nProposal {} under rule {:?}",
        if passed { "PASSED" } else { "FAILED" },
        cli.rule
    );

    Ok(())
}
