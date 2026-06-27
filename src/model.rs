use serde::{Deserialize, Serialize};

fn one() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Workload {
    pub namespace: String,
    pub name: String,
    #[serde(default = "one")]
    pub replicas: u32,
    pub cpu_request: f64,
    pub mem_request: f64,
    pub cpu_usage: f64,
    pub mem_usage: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Pricing {
    pub currency: String,
    pub cpu_hour: f64,
    pub mem_gb_hour: f64,
}

impl Default for Pricing {
    fn default() -> Self {
        Pricing {
            currency: "USD".to_string(),
            cpu_hour: 0.04,
            mem_gb_hour: 0.005,
        }
    }
}

#[derive(Serialize)]
pub struct Finding {
    pub namespace: String,
    pub name: String,
    pub replicas: u32,
    pub monthly_cost: f64,
    pub recommended_cost: f64,
    pub monthly_savings: f64,
    pub cpu_request: f64,
    pub cpu_recommended: f64,
    pub mem_request: f64,
    pub mem_recommended: f64,
    pub flags: Vec<String>,
}

#[derive(Serialize)]
pub struct Report {
    pub currency: String,
    pub total_monthly_cost: f64,
    pub total_monthly_savings: f64,
    pub findings: Vec<Finding>,
}
