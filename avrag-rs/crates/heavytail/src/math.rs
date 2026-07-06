/// Acklam's rational approximation of the inverse normal CDF. |err| < 1.15e-9.
pub fn inv_norm_cdf(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];
    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if !(0.0..1.0).contains(&p) {
        return f64::NAN;
    }

    if p < P_LOW {
        let q = (-2.0 * p.ln()).sqrt();
        let num = ((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5];
        let den = (((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0;
        num / den
    } else if p > P_HIGH {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        let num = ((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5];
        let den = (((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0;
        -num / den
    } else {
        let q = p - 0.5;
        let r = q * q;
        let num = (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q;
        let den = ((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0;
        num / den
    }
}

/// Abramowitz & Stegun 7.1.26 erf approximation → Φ(x). |err| < 7.5e-8.
pub fn norm_cdf(x: f64) -> f64 {
    const A1: f64 = 0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 = 1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 = 1.061405429;
    const P: f64 = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + P * x);
    let y = 1.0
        - (((((A5 * t + A4) * t + A3) * t + A2) * t + A1) * t) * (-x * x).exp();
    0.5 * (1.0 + sign * y)
}

/// OLS intercept and slope for y ~ a + b·x. Returns `(a, b)`.
pub fn linreg(xs: &[f64], ys: &[f64]) -> (f64, f64) {
    let n = xs.len();
    if n == 0 || n != ys.len() {
        return (0.0, 0.0);
    }

    let n_f = n as f64;
    let sum_x: f64 = xs.iter().sum();
    let sum_y: f64 = ys.iter().sum();
    let sum_xy: f64 = xs.iter().zip(ys).map(|(x, y)| x * y).sum();
    let sum_x2: f64 = xs.iter().map(|x| x * x).sum();

    let denom = n_f * sum_x2 - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        return (sum_y / n_f, 0.0);
    }

    let slope = (n_f * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n_f;
    (intercept, slope)
}

#[cfg(test)]
mod tests {
    use super::{inv_norm_cdf, linreg, norm_cdf};

    #[test]
    fn inv_norm_cdf_median_is_zero() {
        assert!((inv_norm_cdf(0.5) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn inv_norm_cdf_99th_percentile() {
        assert!((inv_norm_cdf(0.99) - 2.3263).abs() < 1e-3);
    }

    #[test]
    fn norm_cdf_inverts_inv_norm_cdf_on_grid() {
        for i in 1..100 {
            let p = i as f64 / 100.0;
            let x = inv_norm_cdf(p);
            assert!(
                (norm_cdf(x) - p).abs() < 1e-6,
                "p={p}, x={x}, cdf={}",
                norm_cdf(x)
            );
        }
    }

    #[test]
    fn linreg_hand_computed() {
        // y = 2 + 3x over x = {1, 2, 3, 4}
        let xs = [1.0, 2.0, 3.0, 4.0];
        let ys = [5.0, 8.0, 11.0, 14.0];
        let (intercept, slope) = linreg(&xs, &ys);
        assert!((intercept - 2.0).abs() < 1e-12);
        assert!((slope - 3.0).abs() < 1e-12);
    }
}
