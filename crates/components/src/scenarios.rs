//! Small relational systems used by the demos and the tests.

use metaform_sme::{System, Term};

/// Water flows from a full pipe into an empty beaker; the beaker also holds liquid.
pub fn water_and_heat() -> (System, System) {
    let mut water = System::new(2);
    let greater = water.push("greater", vec![Term::Entity(0), Term::Entity(1)]);
    let flow = water.push("flow", vec![Term::Entity(0), Term::Entity(1)]);
    water.push("cause", vec![Term::Statement(greater), Term::Statement(flow)]);
    water.push("liquid", vec![Term::Entity(0)]);

    let mut heat = System::new(2);
    let greater = heat.push("greater", vec![Term::Entity(0), Term::Entity(1)]);
    let flow = heat.push("flow", vec![Term::Entity(0), Term::Entity(1)]);
    heat.push("cause", vec![Term::Statement(greater), Term::Statement(flow)]);

    (water, heat)
}
