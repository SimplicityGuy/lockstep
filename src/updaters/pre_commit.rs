use std::path::Path;

use crate::context::{Ecosystem, RunCtx};
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct PreCommit;

impl Updater for PreCommit {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::PreCommit
    }
    fn detect(&self, _root: &Path) -> Detection {
        Detection::Absent {
            reason: "not implemented".into(),
        }
    }
    fn run(&self, _ctx: &RunCtx, _det: &Detection) -> UpdateOutcome {
        UpdateOutcome::not_present("not implemented")
    }
}
