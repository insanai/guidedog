//! The `coverage` builder (sphinx.ext.coverage): `python.txt` and `c.txt`, the objects
//! the documents do not describe, and no pages.

use std::sync::Mutex;

use gd_env::BuilderKind;
use gd_host::Problem;
use gd_outputs::{Generation, SITE_UNIT, UnitRecord};
use gd_read::Documented;
use gd_report::Report;
use gd_resolve::{BuildView, OutputBuilder};

/// Writes the coverage reports; keeps the statistics table for the build's summary.
#[derive(Debug, Default)]
pub(crate) struct CoverageBuilder {
    /// The statistics table's lines, when `coverage_statistics_to_stdout` asks for them.
    pub(crate) statistics: Mutex<Vec<String>>,
}

/// What the read documents describe, from the environment.
fn documented(view: &BuildView<'_>) -> Documented {
    let mut d = Documented::default();
    for doc in view.env.docs.iter().filter(|doc| !doc.failed) {
        for o in &doc.objects {
            match o.domain.as_str() {
                "py" => {
                    d.objects.insert(o.fullname.clone());
                }
                "c" => {
                    d.c_items.insert((o.objtype.clone(), o.fullname.clone()));
                }
                _ => {}
            }
        }
        for m in doc.modules.iter().filter(|m| m.domain == "py") {
            d.modules.insert(m.name.clone());
        }
    }
    d
}

impl OutputBuilder for CoverageBuilder {
    fn kind(&self) -> BuilderKind {
        BuilderKind::Coverage
    }

    /// Fails with `build.output` when a report cannot be written.
    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let coverage = gd_read::coverage(view.config, &documented(view));
        reports.extend(coverage.reports);
        let mut unit = generation.unit(SITE_UNIT);
        unit.emit_text("python.txt", &coverage.python)?;
        unit.emit_text("c.txt", &coverage.c)?;
        unit.finish(UnitRecord::default());
        if view.config.ext.coverage.statistics_to_stdout {
            let mut kept =
                self.statistics.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            *kept = coverage.statistics;
        }
        Ok(())
    }
}
