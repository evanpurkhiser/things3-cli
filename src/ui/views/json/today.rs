use chrono::{DateTime, Utc};

use crate::{
    store::{Task, ThingsStore},
    ui::views::json::common::{ResolvedTaskJson, build_tasks_json},
};

pub struct TodayJsonView;

impl TodayJsonView {
    pub fn build(
        tasks: &[Task],
        store: &ThingsStore,
        today: &DateTime<Utc>,
    ) -> Vec<ResolvedTaskJson> {
        build_tasks_json(tasks, store, today)
    }
}
