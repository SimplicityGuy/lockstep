use std::path::Path;

use crate::context::{Ecosystem, RunCtx};
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct PythonRequirements;

impl Updater for PythonRequirements {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Requirements
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
