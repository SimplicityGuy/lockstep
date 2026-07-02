use std::path::Path;

use crate::context::{Ecosystem, RunCtx};
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct Docker;

impl Updater for Docker {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Docker
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
