pub mod math;
pub mod segment;
pub mod tokenize;
pub mod metrics;
pub mod score;
pub mod sensitivity;
pub mod lexops;
pub mod placement;
pub mod workspace;
pub mod patch;
pub mod state;
pub mod llm;
pub mod skeleton;
pub mod draft;
pub mod feedforward;
pub mod compiler;
pub mod refine;
pub mod validator;

/// Carried from v1 spec unchanged (spec §12).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StyleParams {
    pub cv: f64,
    pub phi: f64, // arm-C experiments only
    pub median_length: f64,
    pub hapax_target: f64,
    pub zipf_exponent: f64,
}

impl Default for StyleParams {
    fn default() -> Self {
        Self {
            cv: 0.75,
            phi: 0.4,
            median_length: 20.0,
            hapax_target: 0.45,
            zipf_exponent: 1.0,
        }
    }
}
