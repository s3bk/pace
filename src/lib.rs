use std::sync::atomic::{AtomicUsize, Ordering};
use std::fmt::Display;
use std::sync::{Arc};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[cfg(feature="print")]
pub mod print;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct StageId(usize);

struct ReporterData<'a> {
    stage_ids: AtomicUsize,
    callback: Box<dyn Update + 'a>,
}
impl<'a> ReporterData<'a> {
    fn make_id(&self) -> StageId {
        StageId(self.stage_ids.fetch_add(1, Ordering::Relaxed))
    }
    fn report(&self, id: StageId, r: ProgressReport) {
        self.callback.update(id, r);
    }
}

#[derive(Clone, Serialize)]
struct Progress {
    parent: StageId,
    total: usize,
    count: usize,
    name: String,
}

#[derive(Default, Clone, Serialize)]
pub struct ProgressTracker {
    items: HashMap<StageId, Progress>,
    children: HashMap<StageId, Vec<StageId>>,
}
impl ProgressTracker {
    pub fn update(&mut self, parent: StageId, report: ProgressReport) {
        match report {
            ProgressReport::BeginStage { id, name, steps } => {
                self.items.insert(id, Progress {
                    parent,
                    total: steps,
                    count: 0,
                    name
                });
                self.children.entry(parent).or_default().push(id);
            }
            ProgressReport::EndStage => {
                if let Some(item) = self.items.get(&parent).map(|i| i.parent).and_then(|i| self.items.get_mut(&i)) {
                    item.count += 1;
                }
                self.children.remove(&parent);
            }
            ProgressReport::Progress => {
                if let Some(item) = self.items.get_mut(&parent) {
                    item.count += 1;
                }
            }
        }
    }
    pub fn print(&self) {
        self.print_item(StageId(1), 0);
    }
    fn print_item(&self, id: StageId, level: usize) {
        if let Some(item) = self.items.get(&id) {
            println!("{}{}  {}/{}", Indent(level), item.name, item.count, item.total);
            for &c in self.children.get(&id).iter().flat_map(|v| v.iter()) {
                self.print_item(c, level+1);
            }
        }
    }
}
struct Indent(usize);
impl Display for Indent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for _ in 0..self.0 {
            f.write_str("  ")?;
        }
        Ok(())
    }
}

pub trait Update: Sync + Send {
    fn update(&self, id: StageId, update: ProgressReport);
}

impl Update for () {
    fn update(&self, id: StageId, update: ProgressReport) {}
}

#[derive(Clone)]
pub struct Reporter<'a> {
    shared: Arc<ReporterData<'a>>,
    id: StageId,
}
impl<'a> Reporter<'a> {
    pub fn new(f: impl Update + 'a, steps: usize, name: impl Display) -> Reporter<'a> {
        let shared = Arc::new(ReporterData {
            stage_ids: AtomicUsize::new(1),
            callback: Box::new(f)
        });
        let id = shared.make_id();
        shared.report(StageId(0), ProgressReport::BeginStage { id, name: name.to_string(), steps: steps.into() });

        Reporter {
            id,
            shared, 
        }
    }
    pub fn stage(&self, steps: usize, name: impl Display) -> Reporter<'a> {
        let id = self.shared.make_id();
        self.shared.report(self.id, ProgressReport::BeginStage { id, name: name.to_string(), steps: steps.into() });
        Reporter { id, shared: self.shared.clone() }
    }
    pub fn increment(&self) {
        self.shared.report(self.id, ProgressReport::Progress)
    }
}
impl<'a> Drop for Reporter<'a> {
    fn drop(&mut self) {
        self.shared.report(self.id, ProgressReport::EndStage);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ProgressReport {
    #[serde(rename = "begin")]
    BeginStage { id: StageId, name: String, steps: usize },
    #[serde(rename = "progress")]
    Progress,
    #[serde(rename = "end")]
    EndStage,
}