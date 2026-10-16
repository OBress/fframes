//! Shared contact surfaces for the animal rigs and their habitats.

pub(crate) const ARMADILLO_SOIL: f32 = 785.;

pub(crate) const TAMARIN_BRANCH: &[(f32, f32)] = &[
    (400., -40.),
    (720., 390.),
    (1040., 540.),
    (1150., 540.),
    (1270., 516.),
    (1405., 529.),
    (1640., 610.),
    (1980., 710.),
];

pub(crate) const IGUANA_BRANCH: &[(f32, f32)] = &[
    (-80., 220.),
    (500., 340.),
    (900., 470.),
    (1240., 545.),
    (1530., 675.),
    (2010., 800.),
];

// The two resting feet share this upper leaf edge. The blade extends below it.
pub(crate) const FROG_LEAF_EDGE: &[(f32, f32)] =
    &[(880., 535.), (1166.5, 617.), (1409., 680.5), (1560., 698.)];

pub(crate) fn surface_y(edge: &[(f32, f32)], x: f32) -> f32 {
    for segment in edge.windows(2) {
        let [a, b] = [segment[0], segment[1]];
        if x >= a.0 && x <= b.0 {
            return a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0);
        }
    }
    edge.last().map_or(0., |point| point.1)
}
