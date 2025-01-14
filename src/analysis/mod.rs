pub mod rl_analysis;
pub mod utils;

use std::cell::Cell;

use crate::CliArgs;
use rl_analysis::rl_graph::{RLEdge, RLGraph, RLIndex, RLNode};
use rl_analysis::RLAnalysis;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir;
use rustc_middle::ty;
use serde::de::DeserializeOwned;
use serde::Serialize;
use utils::{TextMod, RL_SERDE_FOLDER};

pub struct Analyzer<'tcx, G>
where
    G: RLGraph + Default + Clone + Serialize,
{
    tcx: ty::TyCtxt<'tcx>,
    cli_args: CliArgs,
    rl_graph: Cell<Option<G>>,
}

impl<'tcx, G> Analyzer<'tcx, G>
where
    G: RLGraph<Node = RLNode, Edge = RLEdge, Index = RLIndex>
        + Default
        + Clone
        + Serialize
        + DeserializeOwned,
{
    pub fn new(tcx: ty::TyCtxt<'tcx>, cli_args: CliArgs) -> Self {
        Self {
            tcx,
            cli_args,
            rl_graph: Cell::new(None),
        }
    }

    fn pre_process_cli_args(&self) {
        log::debug!("Pre-processing CLI arguments");
        if self.cli_args.print_crate {
            log::debug!("Printing the crate");
            let resolver_and_krate = self.tcx.resolver_for_lowering().borrow();
            let krate = &*resolver_and_krate.1;
            println!("{:#?}", krate);
        }

        if self.cli_args.print_mir {
            log::debug!("Printing the MIR");
            mir::write_mir_pretty(self.tcx, None, &mut std::io::stdout())
                .expect("write_mir_pretty failed");
        }
    }

    fn post_process_cli_args(&self) {
        log::debug!("Post-processing CLI arguments");

        if !self.cli_args.print_rl_graph && !self.cli_args.print_serialized_rl_graph {
            log::debug!("No post-processing needed");
            return;
        }

        // let rl_graph: G =
        //     self.deserialize_rl_graph_from_file(&self.tcx.crate_name(LOCAL_CRATE).to_string());
        let rl_graph: G = match self.rl_graph.take() {
            Some(rl_graph) => rl_graph,
            None => RLAnalysis::deserialized_rl_graph_from_file(
                format!(
                    "{}/{}.rlg",
                    RL_SERDE_FOLDER,
                    &self.tcx.crate_name(LOCAL_CRATE).to_string()
                )
                .as_str(),
            ),
        };

        if self.cli_args.print_rl_graph {
            log::debug!("Printing the RustyLinks graph");
            println!("{}", rl_graph.as_dot_str());
        }

        if self.cli_args.print_serialized_rl_graph {
            log::debug!("Printing the serialized RustyLinks graph");
            let serialized = serde_json::to_string(&rl_graph).unwrap();
            println!("{}", serialized);
        }
    }

    fn modify_if_needed(&self, msg: &str, text_mod: TextMod) -> String {
        if self.cli_args.color_log {
            text_mod.apply(msg)
        } else {
            msg.to_string()
        }
    }

    fn run_analysis(&self, name: &str, f: impl FnOnce(&Self)) {
        log::debug!("Running analysis: {}", name);
        f(self);
        log::debug!("Finished analysis: {}", name);
    }

    pub fn run(&self) {
        self.pre_process_cli_args();
        self.run_analysis("RLAnalysis", |analyzer| {
            RLAnalysis::new(analyzer).run();
        });
        self.post_process_cli_args();
    }
}
